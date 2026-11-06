//! The three windows: `main` (cards), `quick` (tray / hotkey panel) and `float` (always-on-top
//! strip). All load the same UI, which picks its layout from the window label.

use std::sync::Arc;

use tauri::window::{Effect, EffectsBuilder};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow, WebviewWindowBuilder, Window, WindowEvent};

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

/// Backdrop material from settings (`tabbed` = Mica Alt, `mica`, `acrylic`, `none`).
/// `fallback` is used for panels where Mica does not suit (the quick panel floats over anything).
fn effects(core: &Core, fallback: Option<Effect>) -> Option<tauri::utils::config::WindowEffectsConfig> {
    let s = core.settings();
    if !s.glass {
        return None;
    }
    let effect = match s.background_effect.as_str() {
        "none" => return None,
        "mica" => Effect::Mica,
        "acrylic" => Effect::Acrylic,
        _ => Effect::Tabbed,
    };
    Some(EffectsBuilder::new().effect(fallback.unwrap_or(effect)).build())
}

fn quick_window(app: &AppHandle) -> Option<WebviewWindow> {
    if let Some(w) = app.get_webview_window("quick") {
        return Some(w);
    }
    let core = app.try_state::<Arc<Core>>()?;
    let mut b = WebviewWindowBuilder::new(app, "quick", WebviewUrl::App("index.html".into()))
        .title("AI Launcher")
        .inner_size(QUICK_W, QUICK_H)
        .decorations(false)
        .transparent(true)
        .resizable(false)
        .skip_taskbar(true)
        .always_on_top(true)
        .shadow(true)
        .disable_drag_drop_handler()
        .visible(false);
    if let Some(e) = effects(&core, Some(Effect::Acrylic)) {
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
        .disable_drag_drop_handler()
        .focused(false)
        .visible(true);
    if let Some((x, y)) = s.float_position {
        b = b.position(x as f64, y as f64);
    }
    match b.build() {
        Ok(w) => {
            if s.float_position.is_none() {
                place_float_default(&w, size + 20.0, size * 3.0);
            }
        }
        Err(e) => log::warn!("float window: {e}"),
    }
}

/// First placement: right edge of the work area, vertically centred. Uses the requested size,
/// because a just-created window may still report 0×0 (Linux).
fn place_float_default(w: &WebviewWindow, width: f64, height: f64) {
    let scale = w.scale_factor().unwrap_or(1.0);
    let (pw, ph) = ((width * scale).round() as i32, (height * scale).round() as i32);
    if let Ok(Some(m)) = w.current_monitor().or_else(|_| w.primary_monitor()) {
        let a = m.work_area();
        let x = a.position.x + a.size.width as i32 - pw - (16.0 * scale) as i32;
        let y = a.position.y + (a.size.height as i32 - ph) / 2;
        let _ = w.set_position(PhysicalPosition::new(x, y));
    }
}

/// Resizes the floating launcher to its content (logical px) without letting it leave the
/// screen: it keeps the edge it sits nearer to (a strip on the right grows to the left), then
/// stays inside the monitor's work area.
pub fn fit_float(w: &WebviewWindow, width: f64, height: f64) {
    let scale = w.scale_factor().unwrap_or(1.0);
    let (nw, nh) = ((width * scale).round() as i32, (height * scale).round() as i32);
    let old_pos = w.outer_position().ok();
    let old_size = w.outer_size().ok().filter(|s| s.width > 1 && s.height > 1);
    // GTK ignores resizes of a non-resizable window (it stays at least 200 px wide), and the
    // builder's setting does not stick on Linux; switch it on here before resizing.
    #[cfg(target_os = "linux")]
    if !w.is_resizable().unwrap_or(true) {
        let _ = w.set_resizable(true);
    }
    let _ = w.set_size(PhysicalSize::new(nw.max(1) as u32, nh.max(1) as u32));
    let Ok(Some(m)) = w.current_monitor().or_else(|_| w.primary_monitor()) else { return };
    let a = m.work_area();
    let (ax, ay, aw, ah) = (a.position.x, a.position.y, a.size.width as i32, a.size.height as i32);
    let (mut x, mut y) = match (old_pos, old_size) {
        (Some(p), Some(s)) => {
            let (ow, oh) = (s.width as i32, s.height as i32);
            let right = p.x + ow / 2 > ax + aw / 2;
            let bottom = p.y + oh / 2 > ay + ah / 2;
            (if right { p.x + ow - nw } else { p.x }, if bottom { p.y + oh - nh } else { p.y })
        }
        (Some(p), None) => (p.x, p.y),
        _ => (ax + aw - nw, ay + (ah - nh) / 2),
    };
    x = x.clamp(ax, (ax + aw - nw).max(ax));
    y = y.clamp(ay, (ay + ah - nh).max(ay));
    if old_pos.map(|p| (p.x, p.y)) != Some((x, y)) {
        let _ = w.set_position(PhysicalPosition::new(x, y));
    }
}

/// Backdrop for windows that already exist (new windows read the setting when created).
pub fn apply_appearance(app: &AppHandle, core: &Arc<Core>) {
    for (label, fallback) in [("main", None), ("quick", Some(Effect::Acrylic))] {
        if let Some(w) = app.get_webview_window(label) {
            let _ = w.set_effects(effects(core, fallback));
        }
    }
}

/// Switches between the main window and the floating launcher. With `floatSwitch = swap`
/// (default) the one being left is hidden; with `both` the main window stays open.
pub fn switch_view(app: &AppHandle, core: &Arc<Core>, to_float: bool) {
    let swap = core.settings().float_switch != "both";
    if to_float {
        if !core.settings().float_enabled {
            let _ = core.mutate(true, |d| {
                d.settings.float_enabled = true;
                Ok(())
            });
        }
        sync_float(app, core);
        if swap {
            hide_main(app);
        }
    } else {
        show_main(app);
        if swap {
            let _ = core.mutate(true, |d| {
                d.settings.float_enabled = false;
                Ok(())
            });
            sync_float(app, core);
        }
    }
}

/// Main ⇄ floating, whichever is showing now.
pub fn toggle_view(app: &AppHandle, core: &Arc<Core>) {
    let main_visible = app.get_webview_window("main").and_then(|w| w.is_visible().ok()).unwrap_or(false);
    let float_visible = app.get_webview_window("float").and_then(|w| w.is_visible().ok()).unwrap_or(false);
    switch_view(app, core, main_visible || !float_visible);
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
