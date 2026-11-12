//! Tray icon: left click opens the quick panel (or the main window), right click shows a menu
//! with every instance so one can be started without opening any window.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use tauri::menu::{Menu, MenuBuilder, MenuItem, PredefinedMenuItem, SubmenuBuilder};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};

use crate::updater::UpdateState;

use crate::core::Core;
use crate::i18n::{lang, t};

const TRAY_ID: &str = "main";

pub fn build(app: &AppHandle, core: &Core) -> tauri::Result<()> {
    let menu = menu(app, core)?;
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(app.default_window_icon().cloned().expect("app icon"))
        .tooltip("AI Launcher")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| on_menu(app, event.id.as_ref()))
        .on_tray_icon_event(|tray, event| on_tray(tray.app_handle(), event))
        .build(app)?;
    Ok(())
}

fn menu(app: &AppHandle, core: &Core) -> tauri::Result<Menu<Wry>> {
    let (settings, items) = {
        let store = core.store.lock();
        let rt = core.runtime.read();
        let mut list: Vec<(String, String, bool)> = store
            .data
            .instances
            .iter()
            .filter(|i| !i.advanced.hidden_in_quick)
            .map(|i| {
                let app_name = store.data.installations.iter().find(|x| x.id == i.installation_id).map(|x| x.name.clone()).unwrap_or_default();
                let label = if i.alias == app_name || app_name.is_empty() { i.alias.clone() } else { format!("{} — {}", i.alias, app_name) };
                (i.id.clone(), label, rt.by_instance.contains_key(&i.id))
            })
            .collect();
        list.truncate(30);
        (store.data.settings.clone(), list)
    };
    let l = lang(&settings);
    let mut sub = SubmenuBuilder::new(app, t(l, "instances"));
    if items.is_empty() {
        sub = sub.item(&MenuItem::with_id(app, "noop", t(l, "empty"), false, None::<&str>)?);
    }
    for (id, label, running) in &items {
        let text = if *running { format!("● {label}") } else { format!("    {label}") };
        sub = sub.item(&MenuItem::with_id(app, format!("inst:{id}"), text, true, None::<&str>)?);
    }
    let mut builder = MenuBuilder::new(app);
    if let UpdateState::Ready { version, .. } = &*core.update.lock() {
        builder = builder
            .item(&MenuItem::with_id(app, "update", t(l, "update.install").replace("{v}", version), true, None::<&str>)?)
            .item(&PredefinedMenuItem::separator(app)?);
    }
    builder
        .item(&MenuItem::with_id(app, "open", t(l, "open"), true, None::<&str>)?)
        .item(&MenuItem::with_id(app, "quick", t(l, "quick"), true, None::<&str>)?)
        .item(&PredefinedMenuItem::separator(app)?)
        .item(&sub.build()?)
        .item(&PredefinedMenuItem::separator(app)?)
        .item(&MenuItem::with_id(app, "settings", t(l, "settings"), true, None::<&str>)?)
        .item(&PredefinedMenuItem::separator(app)?)
        .item(&MenuItem::with_id(app, "quit", t(l, "quit"), true, None::<&str>)?)
        .build()
}

/// Tray clicks. A double click arrives as click → double-click → click, so a single click is
/// acted on only after the double-click interval passes without a double click, and the click
/// that trails a double click is ignored.
static CLICK_GEN: AtomicU64 = AtomicU64::new(0);
static LAST_DOUBLE: Mutex<Option<Instant>> = Mutex::new(None);

fn double_click_ms() -> u64 {
    #[cfg(windows)]
    {
        extern "system" {
            fn GetDoubleClickTime() -> u32;
        }
        (unsafe { GetDoubleClickTime() } as u64).clamp(200, 500)
    }
    #[cfg(not(windows))]
    {
        300
    }
}

fn run_action(app: &AppHandle, action: &str, anchor: Option<(f64, f64, f64, f64)>) {
    match action {
        "main" => crate::windows::show_main(app),
        "quick" | "popup" => crate::windows::toggle_quick(app, anchor),
        _ => {}
    }
}

fn on_tray(app: &AppHandle, event: TrayIconEvent) {
    let Some(core) = app.try_state::<std::sync::Arc<Core>>() else { return };
    let s = core.settings();
    match event {
        TrayIconEvent::DoubleClick { button: MouseButton::Left, .. } => {
            CLICK_GEN.fetch_add(1, Ordering::SeqCst);
            *LAST_DOUBLE.lock() = Some(Instant::now());
            run_action(app, &s.tray_double_click, None);
        }
        TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, rect, .. } => {
            if LAST_DOUBLE.lock().map(|t| t.elapsed() < Duration::from_millis(600)).unwrap_or(false) {
                return;
            }
            let scale = app.get_webview_window("main").and_then(|w| w.scale_factor().ok()).unwrap_or(1.0);
            let pos = rect.position.to_physical::<f64>(scale);
            let size = rect.size.to_physical::<f64>(scale);
            let anchor = Some((pos.x, pos.y, size.width, size.height));
            let action = s.tray_left_click.clone();
            if s.tray_double_click == "none" {
                return run_action(app, &action, anchor);
            }
            let gen = CLICK_GEN.fetch_add(1, Ordering::SeqCst) + 1;
            let app = app.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(double_click_ms()));
                if CLICK_GEN.load(Ordering::SeqCst) == gen {
                    let a = app.clone();
                    let _ = app.run_on_main_thread(move || run_action(&a, &action, anchor));
                }
            });
        }
        _ => {}
    }
}

pub fn refresh(app: &AppHandle, core: &Core) {
    if let (Some(tray), Ok(menu)) = (app.tray_by_id(TRAY_ID), menu(app, core)) {
        let _ = tray.set_menu(Some(menu));
    }
}

fn on_menu(app: &AppHandle, id: &str) {
    let Some(core) = app.try_state::<std::sync::Arc<Core>>() else { return };
    let core = core.inner().clone();
    match id {
        "open" => crate::windows::show_main(app),
        "quick" => crate::windows::show_quick(app, None),
        "settings" => {
            crate::windows::show_main(app);
            use tauri::Emitter;
            let _ = app.emit("navigate", "settings");
        }
        "quit" => app.exit(0),
        "update" => {
            tauri::async_runtime::spawn(async move {
                crate::updater::install(&core).await;
            });
        }
        other => {
            if let Some(inst) = other.strip_prefix("inst:") {
                if let Err(e) = core.launch(inst) {
                    log::warn!("tray launch {inst}: {e}");
                }
            }
        }
    }
}
