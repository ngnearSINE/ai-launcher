//! The three windows: `main` (sidebar and details), `quick` (tray / hotkey panel) and `float`
//! (the always-on-top strip, shown for as long as the launcher runs). All load the same UI,
//! which picks its layout from the window label.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use launcher_core::{Settings, FLOAT_SIZE_MAX, FLOAT_SIZE_MIN};
use tauri::window::{Effect, EffectsBuilder};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow, WebviewWindowBuilder, Window, WindowEvent};

use crate::core::Core;

const QUICK_W: f64 = 440.0;
const QUICK_H: f64 = 560.0;

pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        // Hidden while minimised (taskbar minimise → floating): bring it back on screen first.
        #[cfg(windows)]
        let maximize = w.hwnd().ok().and_then(|h| launcher_platform::win::windows::unpark_hidden(h.0 as isize)).unwrap_or(false);
        let _ = w.unminimize();
        let _ = w.show();
        #[cfg(windows)]
        if maximize {
            let _ = w.maximize();
        }
        let _ = w.set_focus();
    }
}

pub fn hide_main(app: &AppHandle) {
    hide(app, "main");
}

pub fn hide(app: &AppHandle, label: &str) {
    if let Some(w) = app.get_webview_window(label) {
        #[cfg(windows)]
        if let Ok(h) = w.hwnd() {
            launcher_platform::win::windows::remember_placement(h.0 as isize);
        }
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

/// Shape of the floating strip at 100 % (logical px), reported by its page: a fixed part
/// (handle, footer, padding) plus `total` cells of `cell` along its axis, `across` thick.
#[derive(Debug, Clone, Copy, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FloatLayout {
    pub vertical: bool,
    pub fixed: f64,
    pub cell: f64,
    pub across: f64,
    pub total: usize,
    /// The strip's shape floating and docked, whichever it is now (for the ghosts that preview
    /// the other one). Zero from an older page: the current shape is used instead.
    #[serde(default)]
    pub float_vertical: bool,
    #[serde(default)]
    pub float_fixed: f64,
    #[serde(default)]
    pub float_across: f64,
    #[serde(default)]
    pub dock_fixed: f64,
    #[serde(default)]
    pub dock_across: f64,
}

impl FloatLayout {
    /// The same strip floating.
    pub fn floating(&self) -> FloatLayout {
        if self.float_across > 0.0 {
            FloatLayout { vertical: self.float_vertical, fixed: self.float_fixed, across: self.float_across, ..*self }
        } else {
            *self
        }
    }
    /// The same strip docked (always horizontal).
    pub fn docked(&self) -> FloatLayout {
        if self.dock_across > 0.0 {
            FloatLayout { vertical: false, fixed: self.dock_fixed, across: self.dock_across, ..*self }
        } else {
            *self
        }
    }
}

/// `float_size` that means 100 %.
const FLOAT_BASE: f64 = 44.0;
pub(crate) static LAYOUT: parking_lot::Mutex<Option<FloatLayout>> = parking_lot::Mutex::new(None);
/// The user is resizing the strip: our own fitting stays out of the way until it settles.
static RESIZING: AtomicBool = AtomicBool::new(false);
static RESIZE_GEN: AtomicU64 = AtomicU64::new(0);
#[cfg(not(windows))]
static MOVE_GEN: AtomicU64 = AtomicU64::new(0);

pub(crate) fn float_zoom(s: &Settings) -> f64 {
    s.float_size as f64 / FLOAT_BASE
}

/// Creates the floating launcher, or brings it in line with the settings. It is always there
/// while the launcher runs.
pub fn sync_float(app: &AppHandle, core: &Arc<Core>) {
    let s = core.settings();
    #[cfg(windows)]
    launcher_platform::win::magnet::configure(if s.float_snap_edges { s.float_snap_distance as i32 } else { 0 }, s.float_snap_windows);
    if let Some(w) = app.get_webview_window("float") {
        let _ = w.set_always_on_top(s.float_always_on_top);
        let _ = w.show();
        refit_float(&w, core);
        return;
    }
    let mut b = WebviewWindowBuilder::new(app, "float", WebviewUrl::App("index.html".into()))
        .title("AI Launcher")
        .inner_size(64.0, 200.0)
        .decorations(false)
        .transparent(true)
        // Resizable so its corners scale it and its ends add or remove cells (see `magnet`).
        .resizable(true)
        .maximizable(false)
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
            let _ = w.set_zoom(float_zoom(&s));
            #[cfg(windows)]
            if let Ok(h) = w.hwnd() {
                launcher_platform::win::magnet::install(h.0 as isize);
                let app = app.clone();
                let app_moving = app.clone();
                launcher_platform::win::magnet::on_drop(move |_, _, start| {
                    let app2 = app.clone();
                    let _ = app.run_on_main_thread(move || crate::dock::on_float_dropped(&app2, start));
                });
                // Already on the window's thread, inside the move loop.
                launcher_platform::win::magnet::on_moving(move |x, y| crate::dock::on_float_moving(&app_moving, x, y));
            }
            if s.float_position.is_none() {
                place_float_default(&w, 64.0, 200.0);
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

/// The page's new shape (items added, orientation changed…).
pub fn set_float_layout(w: &WebviewWindow, core: &Arc<Core>, layout: FloatLayout) {
    *LAYOUT.lock() = Some(layout);
    refit_float(w, core);
}

/// Logical size for the current scale and number of cells (never longer than the work area).
pub(crate) fn float_size_for(l: &FloatLayout, s: &Settings, area_len: Option<f64>) -> (f64, f64) {
    let k = float_zoom(s);
    let mut n = if s.float_slots == 0 { l.total } else { (s.float_slots as usize).min(l.total) }.max(1);
    if let (Some(len), true) = (area_len, l.cell > 0.0) {
        let fit = ((len / k - l.fixed) / l.cell).floor().max(1.0) as usize;
        n = n.min(fit);
    }
    let along = (l.fixed + n as f64 * l.cell) * k;
    let across = l.across * k;
    if l.vertical {
        (across, along)
    } else {
        (along, across)
    }
}

/// Sizes the strip to its layout, zoom and cell count without letting it leave the screen: it
/// keeps the edge it sits nearer to (a strip on the right grows to the left), then stays inside
/// the monitor's work area.
fn refit_float(w: &WebviewWindow, core: &Arc<Core>) {
    if RESIZING.load(Ordering::Relaxed) {
        return;
    }
    let Some(l) = *LAYOUT.lock() else { return };
    let s = core.settings();
    #[cfg(windows)]
    if s.float_docked {
        crate::dock::refit_docked(w, core, &l, 0);
        return;
    }
    // Always the floating shape, also in the moment before the page reports it after leaving
    // the taskbar: the same size the ghost showed.
    let l = l.floating();
    let scale = w.scale_factor().unwrap_or(1.0);
    let k = float_zoom(&s);
    let _ = w.set_zoom(k);
    let monitor = w.current_monitor().ok().flatten().or_else(|| w.primary_monitor().ok().flatten());
    let area_len = monitor.as_ref().map(|m| {
        let a = m.work_area();
        (if l.vertical { a.size.height } else { a.size.width }) as f64 / scale
    });
    let (width, height) = float_size_for(&l, &s, area_len);
    #[cfg(windows)]
    launcher_platform::win::magnet::set_strip(launcher_platform::snap::Strip {
        vertical: l.vertical,
        fixed: l.fixed * k * scale,
        cell: l.cell * k * scale,
        total: l.total,
        min_across: (l.across * (FLOAT_SIZE_MIN as f64 / FLOAT_BASE) * scale).round() as i32,
        max_across: (l.across * (FLOAT_SIZE_MAX as f64 / FLOAT_BASE) * scale).round() as i32,
    });
    let (nw, nh) = ((width * scale).round() as i32, (height * scale).round() as i32);
    let old_pos = w.outer_position().ok();
    let old_size = w.outer_size().ok().filter(|s| s.width > 1 && s.height > 1);
    let _ = w.set_size(PhysicalSize::new(nw.max(1) as u32, nh.max(1) as u32));
    #[cfg(windows)]
    if launcher_platform::win::magnet::in_size_move() {
        return; // being moved: the user places it
    }
    let Some(m) = monitor else { return };
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


/// A size change we did not make: the user is dragging a corner (scale) or an end (cells).
/// While it lasts the page zoom follows the thickness live; once it settles the result is saved
/// as `float_size` or `float_slots` and the strip is fitted exactly.
fn on_float_resized(w: &WebviewWindow, core: &Arc<Core>, size: PhysicalSize<u32>) {
    if core.settings().float_docked {
        return; // the taskbar's size, not the user's
    }
    let Some(l) = LAYOUT.lock().map(|l| l.floating()) else { return };
    let scale = w.scale_factor().unwrap_or(1.0);
    let (nw, nh) = (size.width as i64, size.height as i64);
    let resizing = RESIZING.load(Ordering::Relaxed);
    // Windows says when the user holds the frame; elsewhere, a size we did not ask for.
    #[cfg(windows)]
    let user = launcher_platform::win::magnet::in_user_resize();
    #[cfg(not(windows))]
    let user = {
        let area = w.current_monitor().ok().flatten().map(|m| (if l.vertical { m.work_area().size.height } else { m.work_area().size.width }) as f64 / scale);
        let (ew, eh) = float_size_for(&l, &core.settings(), area);
        (nw - (ew * scale).round() as i64).abs() > 2 || (nh - (eh * scale).round() as i64).abs() > 2
    };
    if !(resizing || user) || nw <= 1 || nh <= 1 {
        return;
    }
    RESIZING.store(true, Ordering::Relaxed);
    let across = if l.vertical { nw } else { nh } as f64 / scale;
    if l.across > 0.0 {
        let _ = w.set_zoom((across / l.across).clamp(FLOAT_SIZE_MIN as f64 / FLOAT_BASE, FLOAT_SIZE_MAX as f64 / FLOAT_BASE));
    }
    let gen = RESIZE_GEN.fetch_add(1, Ordering::Relaxed) + 1;
    let (w, core) = (w.clone(), core.clone());
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            if RESIZE_GEN.load(Ordering::Relaxed) != gen {
                return; // a newer resize event will settle it
            }
            #[cfg(windows)]
            if launcher_platform::win::magnet::in_size_move() {
                continue; // button still held
            }
            break;
        }
        settle_float_resize(&w, &core, l);
    });
}

fn settle_float_resize(w: &WebviewWindow, core: &Arc<Core>, l: FloatLayout) {
    let scale = w.scale_factor().unwrap_or(1.0);
    let Ok(size) = w.inner_size() else { return };
    let (wl, hl) = (size.width as f64 / scale, size.height as f64 / scale);
    let (along, across) = if l.vertical { (hl, wl) } else { (wl, hl) };
    let s = core.settings();
    let k_old = float_zoom(&s);
    let k_new = if l.across > 0.0 { across / l.across } else { k_old };
    // A corner may change both the scale and the cells, an end only the cells: save both.
    let size = (FLOAT_BASE * k_new).round().clamp(FLOAT_SIZE_MIN as f64, FLOAT_SIZE_MAX as f64) as u16;
    let k = size as f64 / FLOAT_BASE;
    let _ = core.mutate(true, |d| {
        d.settings.float_size = size;
        if l.cell > 0.0 && l.total > 0 {
            let n = ((along / k - l.fixed) / l.cell).round().clamp(1.0, l.total as f64) as usize;
            d.settings.float_slots = if n >= l.total { 0 } else { n as u16 };
        }
        Ok(())
    });
    let s = core.settings();
    log::info!("float resized → size {} slots {}", s.float_size, s.float_slots);
    RESIZING.store(false, Ordering::Relaxed);
    refit_float(w, core);
}

/// Linux has no live magnet: once a move pauses, pull the strip onto nearby work-area edges.
#[cfg(not(windows))]
fn snap_after_move(w: &WebviewWindow, core: &Arc<Core>) {
    let s = core.settings();
    if !s.float_snap_edges {
        return;
    }
    let gen = MOVE_GEN.fetch_add(1, Ordering::Relaxed) + 1;
    let w = w.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(350)).await;
        if MOVE_GEN.load(Ordering::Relaxed) != gen {
            return;
        }
        use launcher_platform::snap::{snap, Rect};
        let (Ok(p), Ok(sz)) = (w.outer_position(), w.outer_size()) else { return };
        let r = Rect::new(p.x, p.y, p.x + sz.width as i32, p.y + sz.height as i32);
        let areas: Vec<Rect> = w
            .available_monitors()
            .unwrap_or_default()
            .iter()
            .map(|m| {
                let a = m.work_area();
                Rect::new(a.position.x, a.position.y, a.position.x + a.size.width as i32, a.position.y + a.size.height as i32)
            })
            .collect();
        let dist = (s.float_snap_distance as f64 * w.scale_factor().unwrap_or(1.0)) as i32;
        let to = snap(r, &areas, &[], dist);
        if to != r {
            let _ = w.set_position(PhysicalPosition::new(to.left, to.top));
        }
    });
}

/// Backdrop for windows that already exist (new windows read the setting when created).
pub fn apply_appearance(app: &AppHandle, core: &Arc<Core>) {
    for (label, fallback) in [("main", None), ("quick", Some(Effect::Acrylic))] {
        if let Some(w) = app.get_webview_window(label) {
            let _ = w.set_effects(effects(core, fallback));
        }
    }
}

/// Opens the main window, or hides it when it is already open (global hotkey).
pub fn toggle_main(app: &AppHandle) {
    let open = app
        .get_webview_window("main")
        .map(|w| w.is_visible().unwrap_or(false) && !w.is_minimized().unwrap_or(false))
        .unwrap_or(false);
    if open {
        hide(app, "main");
    } else {
        show_main(app);
    }
}

pub fn on_event(window: &Window, event: &WindowEvent) {
    let app = window.app_handle();
    let Some(core) = app.try_state::<Arc<Core>>() else { return };
    match (window.label(), event) {
        // x closes the window only; the launcher keeps running with its floating strip.
        ("main", WindowEvent::CloseRequested { api, .. }) => {
            api.prevent_close();
            hide(app, "main");
        }
        ("quick", WindowEvent::Focused(false)) => {
            let _ = window.hide();
        }
        ("float", WindowEvent::Moved(_)) if core.settings().float_docked => {}
        ("float", WindowEvent::Destroyed) => {
            let (app, core) = (app.clone(), core.inner().clone());
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
                if app.get_webview_window("float").is_none() {
                    log::info!("float window was destroyed (Explorer restart?) → recreating");
                    sync_float(&app, &core);
                }
            });
        }
        ("float", WindowEvent::Moved(pos)) => {
            {
                let mut store = core.store.lock();
                store.data.settings.float_position = Some((pos.x, pos.y));
            }
            core.inner().save_soon();
            #[cfg(not(windows))]
            if let Some(w) = app.get_webview_window("float") {
                snap_after_move(&w, core.inner());
            }
        }
        ("float", WindowEvent::Resized(size)) => {
            if let Some(w) = app.get_webview_window("float") {
                on_float_resized(&w, core.inner(), *size);
            }
        }
        _ => {}
    }
}
