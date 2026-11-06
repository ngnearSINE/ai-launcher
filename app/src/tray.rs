//! Tray icon: left click opens the quick panel (or the main window), right click shows a menu
//! with every instance so one can be started without opening any window.

use tauri::menu::{Menu, MenuBuilder, MenuItem, PredefinedMenuItem, SubmenuBuilder};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};

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
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, rect, .. } = event {
                let app = tray.app_handle();
                let Some(core) = app.try_state::<std::sync::Arc<Core>>() else { return };
                if core.settings().tray_left_click == "main" {
                    crate::windows::show_main(app);
                } else {
                    let scale = app.get_webview_window("main").and_then(|w| w.scale_factor().ok()).unwrap_or(1.0);
                    let pos = rect.position.to_physical::<f64>(scale);
                    let size = rect.size.to_physical::<f64>(scale);
                    crate::windows::toggle_quick(app, Some((pos.x, pos.y, size.width, size.height)));
                }
            }
        })
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
    let float_label = if settings.float_enabled { t(l, "float.hide") } else { t(l, "float.show") };
    MenuBuilder::new(app)
        .item(&MenuItem::with_id(app, "open", t(l, "open"), true, None::<&str>)?)
        .item(&MenuItem::with_id(app, "quick", t(l, "quick"), true, None::<&str>)?)
        .item(&PredefinedMenuItem::separator(app)?)
        .item(&sub.build()?)
        .item(&PredefinedMenuItem::separator(app)?)
        .item(&MenuItem::with_id(app, "float", float_label, true, None::<&str>)?)
        .item(&MenuItem::with_id(app, "settings", t(l, "settings"), true, None::<&str>)?)
        .item(&PredefinedMenuItem::separator(app)?)
        .item(&MenuItem::with_id(app, "quit", t(l, "quit"), true, None::<&str>)?)
        .build()
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
        "float" => {
            let on = !core.settings().float_enabled;
            let _ = core.mutate(true, |d| {
                d.settings.float_enabled = on;
                Ok(())
            });
            crate::windows::sync_float(app, &core);
        }
        "quit" => app.exit(0),
        other => {
            if let Some(inst) = other.strip_prefix("inst:") {
                if let Err(e) = core.launch(inst) {
                    log::warn!("tray launch {inst}: {e}");
                }
            }
        }
    }
}
