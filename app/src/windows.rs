//! The three windows: `main` (cards), `quick` (tray / hotkey panel) and `float` (always-on-top
//! strip). All load the same UI, which picks its layout from the window label.

use std::sync::Arc;

use tauri::window::{Effect, EffectsBuilder};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, WebviewUrl, WebviewWindow, WebviewWindowBuilder, Window, WindowEvent};

use crate::core::Core;

const QUICK_W: f64 = 380.0;
const QUICK_H: f64 = 560.0;

pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

pub fn hide_main(app: &AppHandle) {
    hide(app, "main");
}

pub fn hide(app: &AppHandle, label: &str) {
    if let Some(w) = app.get_webview_window(label) {
        let _ = w.hide();
    }
}

fn effects(glass: bool, effect: Effect) -> Option<tauri::utils::config::WindowEffectsConfig> {
    glass.then(|| EffectsBuilder::new().effect(effect).build())
}

fn quick_window(app: &AppHandle) -> Option<WebviewWindow> {
    if let Some(w) = app.get_webview_window("quick") {
        return Some(w);
    }
    let core = app.try_state::<Arc<Core>>()?;
    let glass = core.settings().glass;
    let mut b = WebviewWindowBuilder::new(app, "quick", WebviewUrl::App("index.html".into()))
        .title("AI Launcher")
        .inner_size(QUICK_W, QUICK_H)
        .decorations(false)
        .transparent(true)
        .resizable(false)
        .skip_taskbar(true)
        .always_on_top(true)
        .shadow(true)
        .visible(false);
    if let Some(e) = effects(glass, Effect::Acrylic) {
        b = b.effects(e);
    }
    b.build().ok()
}

/// Opens the quick panel next to the tray icon (`anchor` = tray icon rect, physical px) or at
/// the centre of the monitor under the cursor.
pub fn show_quick(app: &AppHandle, anchor: Option<(f64, f64, f64, f64)>) {
    let Some(w) = quick_window(app) else { return };
    let scale = w.scale_factor().unwrap_or(1.0);
    let (qw, qh) = (QUICK_W * scale, QUICK_H * scale);
    let point = match anchor {
        Some((x, y, _, _)) => (x, y),
        None => app.cursor_position().map(|p| (p.x, p.y)).unwrap_or((0.0, 0.0)),
    };
    if let Ok(Some(m)) = app.monitor_from_point(point.0, point.1) {
        let area = m.work_area();
        let (ax, ay) = (area.position.x as f64, area.position.y as f64);
        let (aw, ah) = (area.size.width as f64, area.size.height as f64);
        let (x, y) = match anchor {
            Some((tx, ty, tw, _th)) => {
                let margin = 12.0 * scale;
                let x = (tx + tw / 2.0 - qw / 2.0).clamp(ax + margin, ax + aw - qw - margin);
                // Above a bottom taskbar, below a top one.
                let y = if ty > ay + ah / 2.0 { ay + ah - qh - margin } else { ay + margin };
                (x, y)
            }
            None => (ax + (aw - qw) / 2.0, ay + (ah - qh) / 3.0),
        };
        let _ = w.set_position(PhysicalPosition::new(x as i32, y as i32));
    }
    let _ = w.emit("quick-opened", ());
    let _ = w.show();
    let _ = w.set_focus();
}

pub fn toggle_quick(app: &AppHandle, anchor: Option<(f64, f64, f64, f64)>) {
    if let Some(w) = app.get_webview_window("quick") {
        if w.is_visible().unwrap_or(false) {
            let _ = w.hide();
            return;
        }
    }
    show_quick(app, anchor);
}

/// Creates or removes the floating launcher to match the setting.
pub fn sync_float(app: &AppHandle, core: &Arc<Core>) {
    let s = core.settings();
    let existing = app.get_webview_window("float");
    if !s.float_enabled {
        if let Some(w) = existing {
            let _ = w.close();
        }
        return;
    }
    if let Some(w) = existing {
        let _ = w.set_always_on_top(s.float_always_on_top);
        let _ = w.show();
        return;
    }
    let size = s.float_size as f64;
    let mut b = WebviewWindowBuilder::new(app, "float", WebviewUrl::App("index.html".into()))
        .title("AI Launcher")
        .inner_size(size + 20.0, size * 3.0)
        .decorations(false)
        .transparent(true)
        .resizable(false)
        .skip_taskbar(true)
        .always_on_top(s.float_always_on_top)
        .shadow(false)
        .focused(false)
        .visible(true);
    if let Some((x, y)) = s.float_position {
        b = b.position(x as f64, y as f64);
    }
    match b.build() {
        Ok(w) => {
            if s.float_position.is_none() {
                place_float_default(&w);
            }
        }
        Err(e) => log::warn!("float window: {e}"),
    }
}

fn place_float_default(w: &WebviewWindow) {
    if let (Ok(Some(m)), Ok(size)) = (w.current_monitor(), w.outer_size()) {
        let a = m.work_area();
        let x = a.position.x + a.size.width as i32 - size.width as i32 - 16;
        let y = a.position.y + (a.size.height as i32 - size.height as i32) / 2;
        let _ = w.set_position(PhysicalPosition::new(x, y));
    }
}

/// Glass on/off for windows that already exist (new windows read the setting when created).
pub fn apply_appearance(app: &AppHandle, core: &Arc<Core>) {
    let glass = core.settings().glass;
    for (label, effect) in [("main", Effect::Mica), ("quick", Effect::Acrylic)] {
        if let Some(w) = app.get_webview_window(label) {
            let _ = w.set_effects(effects(glass, effect));
        }
    }
}

pub fn on_event(window: &Window, event: &WindowEvent) {
    let app = window.app_handle();
    let Some(core) = app.try_state::<Arc<Core>>() else { return };
    match (window.label(), event) {
        ("main", WindowEvent::CloseRequested { api, .. }) => {
            if core.settings().close_to_tray {
                api.prevent_close();
                let _ = window.hide();
            }
        }
        ("quick", WindowEvent::Focused(false)) => {
            let _ = window.hide();
        }
        ("float", WindowEvent::Moved(pos)) => {
            {
                let mut store = core.store.lock();
                store.data.settings.float_position = Some((pos.x, pos.y));
            }
            core.inner().save_soon();
        }
        _ => {}
    }
}
