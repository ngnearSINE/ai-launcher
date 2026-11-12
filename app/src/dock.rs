//! The floating strip in and out of the taskbar (Windows).
//!
//! One idea runs through all of it: a gesture only ever decides a [`Landing`] — where the strip
//! will be when the mouse is let go. The ghost draws exactly that, and on release [`land`] puts
//! the window exactly there and saves the settings from the same values. Nothing recomputes the
//! spot afterwards, so what the user saw is what they get (2026-11-12: a ghost, a drop handler
//! and a later fit each working the place out on their own disagreed and lost the strip).
//!
//! - Moving the floating strip: over a taskbar the landing is a dock slot (ghost in the
//!   taskbar); elsewhere there is none and the window itself is where it goes.
//! - Dragging the docked strip by its handle: inside the taskbar the landing is the slot under
//!   the cursor and the strip itself slides there; pulled out of it, the landing is a floating
//!   rectangle (ghost of the floating bar).
//! - "Undock" in a menu: the landing is the floating rectangle it came from.
//!
//! Between gestures the strip keeps its anchor (icons' end, widgets, notification area…): a
//! follower re-places it every 1.5 s when the taskbar changes, and a refit only changes its
//! width, holding the anchored edge.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use launcher_core::Settings;
use launcher_platform::dock_slot::{slot_x, snap_slot, DockAnchor, DockMarks};
use launcher_platform::snap::{snap, Rect};
use launcher_platform::win::dock::{self as tb_dock, TaskbarSpace};
use launcher_platform::win::ghost;
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewWindow};

use crate::core::Core;
use crate::windows::{float_size_for, float_zoom, FloatLayout, LAYOUT};

/// Space between the strip and the landmark it sits against (logical px).
const DOCK_GAP: f64 = 4.0;
/// Pulled this far (logical px) above or below the taskbar, the docked strip would float.
const PULL_OUT: f64 = 28.0;
/// Where the floating strip is held by its handle (logical px at 100 %, from its top-left).
const GRAB: (f64, f64) = (30.0, 18.0);

/// Where the strip will be when the mouse is let go.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Landing {
    /// In `taskbar`, left edge at client `x`, `width` wide (physical px), kept beside `anchor`.
    Dock { taskbar: isize, anchor: DockAnchor, x: i32, width: i32 },
    /// Floating, exactly over `rect` (physical screen px).
    Float { rect: Rect },
}

impl Landing {
    fn is_dock(&self) -> bool {
        matches!(self, Landing::Dock { .. })
    }
    /// The rectangle on screen (what the ghost covers).
    fn screen_rect(&self) -> Option<Rect> {
        match *self {
            Landing::Float { rect } => Some(rect),
            Landing::Dock { taskbar, x, width, .. } => {
                let bar = tb_dock::taskbar_rect(taskbar)?;
                let sp = tb_dock::space(taskbar)?;
                Some(Rect::new(bar.left + x, bar.top, bar.left + x + width, bar.top + sp.height))
            }
        }
    }
}

/// The landing of the gesture in progress (what the ghost shows), if any.
static LANDING: parking_lot::Mutex<Option<Landing>> = parking_lot::Mutex::new(None);
/// A gesture on the docked strip is running: the follower and refits keep out of its way.
static DRAGGING: AtomicBool = AtomicBool::new(false);
/// The taskbar the strip was last docked into (for re-docking after Explorer restarts).
static DOCKED_ON: parking_lot::Mutex<Option<isize>> = parking_lot::Mutex::new(None);

// ---- measures ---------------------------------------------------------------------------

fn scale_of(sp: &TaskbarSpace) -> f64 {
    sp.dpi as f64 / 96.0
}

fn gap(sp: &TaskbarSpace) -> i32 {
    (DOCK_GAP * scale_of(sp)).round() as i32
}

/// Magnet reach inside the taskbar (physical px); 0 when the magnet is off.
fn snap_dist(s: &Settings, sp: &TaskbarSpace) -> i32 {
    if s.float_snap_edges { (s.float_snap_distance as f64 * scale_of(sp)).round() as i32 } else { 0 }
}

/// Magnet reach for a strip coming in from outside: three times the usual, because it is
/// placed centred under the cursor, which is rarely that exact.
fn drop_dist(s: &Settings, sp: &TaskbarSpace) -> i32 {
    snap_dist(s, sp) * 3
}

/// A free spot is saved in logical px; keep it to one that reads back to the same pixel.
fn exact_slot(anchor: DockAnchor, x: i32, sp: &TaskbarSpace) -> i32 {
    if anchor == DockAnchor::Free {
        let k = scale_of(sp);
        ((x as f64 / k).round() * k).round() as i32
    } else {
        x
    }
}

fn accent_rgb(s: &Settings) -> u32 {
    u32::from_str_radix(s.accent.trim_start_matches('#'), 16).unwrap_or(0x0A84FF)
}

/// How the strip fits a taskbar: page zoom, width and cells (physical px).
#[derive(Clone, Copy)]
pub struct DockFit {
    pub k: f64,
    pub width: i32,
    /// Cells shown when the user has not chosen (`float_dock_slots` = 0).
    pub auto_n: usize,
    /// Most cells the user may ask for.
    pub max_n: usize,
    pub fixed_px: f64,
    pub cell_px: f64,
}

pub fn dock_fit(l: &FloatLayout, s: &Settings, sp: &TaskbarSpace) -> DockFit {
    let l = l.docked();
    let scale = scale_of(sp);
    let k = sp.height as f64 / (l.across * scale).max(1.0);
    let (fixed_px, cell_px) = (l.fixed * k * scale, (l.cell * k * scale).max(1.0));
    // On its own about half of the taskbar; as many as the user asks for, up to 90 %.
    let room = |share: f64| (((sp.right as f64 * share) - fixed_px) / cell_px).floor().max(1.0) as usize;
    let total = l.total.max(1);
    let auto_n = total.min(room(0.45));
    let max_n = total.min(room(0.9)).max(auto_n);
    let n = if s.float_dock_slots == 0 { auto_n } else { (s.float_dock_slots as usize).clamp(1, max_n) };
    let width = (fixed_px + n as f64 * cell_px).round() as i32;
    DockFit { k, width, auto_n, max_n, fixed_px, cell_px }
}

/// Landmarks of a taskbar: the last ones read (UI Automation takes ~50–100 ms, too slow for
/// every mouse step), refreshed in the background; until then only the notification area and
/// other apps' windows (both cheap).
static MARKS: parking_lot::Mutex<Option<(isize, DockMarks, std::time::Instant)>> = parking_lot::Mutex::new(None);
static MARKS_BUSY: AtomicBool = AtomicBool::new(false);

fn cached_marks(tb: isize, sp: &TaskbarSpace) -> DockMarks {
    let cached = MARKS.lock().clone();
    let fresh = cached.as_ref().is_some_and(|(t, _, at)| *t == tb && at.elapsed().as_secs() < 2);
    if !fresh && !MARKS_BUSY.swap(true, Ordering::SeqCst) {
        std::thread::spawn(move || {
            launcher_platform::win::com_init();
            if let Some(m) = tb_dock::landmarks(tb) {
                *MARKS.lock() = Some((tb, m, std::time::Instant::now()));
            }
            MARKS_BUSY.store(false, Ordering::SeqCst);
        });
    }
    match cached {
        Some((t, m, _)) if t == tb => m,
        _ => DockMarks { icons: None, widgets: None, tray: sp.right, others: tb_dock::others(tb) },
    }
}

/// Landmarks read now (off the window thread), remembered for the next gesture.
fn fresh_marks(tb: isize, sp: &TaskbarSpace) -> DockMarks {
    match tb_dock::landmarks(tb) {
        Some(m) => {
            *MARKS.lock() = Some((tb, m.clone(), std::time::Instant::now()));
            m
        }
        None => DockMarks { icons: None, widgets: None, tray: sp.right, others: tb_dock::others(tb) },
    }
}

// ---- landings ---------------------------------------------------------------------------

/// The floating strip's rectangle with its top-left at `at`, sized for the monitor holding
/// `on` and kept inside its work area.
fn float_rect(monitors: &[tauri::Monitor], s: &Settings, l: &FloatLayout, at: (i32, i32), on: (i32, i32)) -> Option<Rect> {
    let inside = |m: &&tauri::Monitor| {
        let (p, z) = (m.position(), m.size());
        on.0 >= p.x && on.0 < p.x + z.width as i32 && on.1 >= p.y && on.1 < p.y + z.height as i32
    };
    let m = monitors.iter().find(inside).or(monitors.first())?;
    let scale = m.scale_factor();
    let a = m.work_area();
    let l = l.floating();
    let area_len = (if l.vertical { a.size.height } else { a.size.width }) as f64 / scale;
    let (w, h) = float_size_for(&l, s, Some(area_len));
    let (w, h) = ((w * scale).round() as i32, (h * scale).round() as i32);
    let (ax, ay, aw, ah) = (a.position.x, a.position.y, a.size.width as i32, a.size.height as i32);
    let x = at.0.clamp(ax, (ax + aw - w).max(ax));
    let y = at.1.clamp(ay, (ay + ah - h).max(ay));
    Some(Rect::new(x, y, x + w, y + h))
}

/// Floating, held by its handle under the cursor and pulled to the work area's edges.
fn float_landing_at(monitors: &[tauri::Monitor], s: &Settings, l: &FloatLayout, cx: i32, cy: i32) -> Option<Landing> {
    let scale = monitors
        .iter()
        .find(|m| {
            let (p, z) = (m.position(), m.size());
            cx >= p.x && cx < p.x + z.width as i32 && cy >= p.y && cy < p.y + z.height as i32
        })
        .map_or(1.0, |m| m.scale_factor());
    let k = float_zoom(s) * scale;
    let at = (cx - (GRAB.0 * k) as i32, cy - (GRAB.1 * k) as i32);
    let rect = float_rect(monitors, s, l, at, (cx, cy))?;
    let dist = if s.float_snap_edges { (s.float_snap_distance as f64 * scale) as i32 } else { 0 };
    Some(Landing::Float { rect: snap(rect, &launcher_platform::win::magnet::work_areas(), &[], dist) })
}

/// Docked into the taskbar under the cursor: centred on it, then the magnet decides.
fn dock_landing_at(s: &Settings, l: &FloatLayout, cx: i32, cy: i32) -> Option<Landing> {
    let tb = tb_dock::taskbar_at(cx, cy)?;
    let (sp, bar) = (tb_dock::space(tb)?, tb_dock::taskbar_rect(tb)?);
    let fit = dock_fit(l, s, &sp);
    let marks = cached_marks(tb, &sp);
    let (anchor, x) = snap_slot(cx - bar.left - fit.width / 2, fit.width, &marks, gap(&sp), drop_dist(s, &sp));
    Some(Landing::Dock { taskbar: tb, anchor, x: exact_slot(anchor, x, &sp), width: fit.width })
}

/// Records the gesture's landing and shows its ghost — only when it would change the strip's
/// mode (floating ↔ docked); within a mode the strip itself shows where it goes.
fn preview(s: &Settings, landing: Option<Landing>, strip_docked: bool) {
    let mut cur = LANDING.lock();
    if *cur == landing {
        return;
    }
    *cur = landing;
    match landing.filter(|l| l.is_dock() != strip_docked).and_then(|l| l.screen_rect()) {
        Some(r) => ghost::show((r.left, r.top, r.right, r.bottom), accent_rgb(s)),
        None => ghost::hide(),
    }
}

fn clear_preview() {
    *LANDING.lock() = None;
    ghost::hide();
}

/// Puts the strip exactly at `landing` and saves it. `came_from`: where a floating strip was
/// before it docked (it returns there when undocked from a menu). On the window's thread.
pub fn land(app: &AppHandle, core: &Arc<Core>, landing: Landing, came_from: Option<(i32, i32)>) {
    clear_preview();
    let Some(w) = app.get_webview_window("float") else { return finish_gesture() };
    let Some(hwnd) = w.hwnd().ok().map(|h| h.0 as isize) else { return finish_gesture() };
    let s = core.settings();
    match landing {
        Landing::Dock { taskbar, anchor, x, width } => {
            let Some(sp) = tb_dock::space(taskbar) else { return finish_gesture() };
            if let Some(l) = *LAYOUT.lock() {
                let _ = w.set_zoom(dock_fit(&l, &s, &sp).k);
            }
            // The window first, then the settings: the page re-lays out on the settings change
            // and its refit then finds the strip already where it belongs.
            if tb_dock::parent_taskbar(hwnd) == Some(taskbar) {
                tb_dock::place_docked(hwnd, x, width);
            } else {
                tb_dock::undock(hwnd);
                if !tb_dock::dock(hwnd, taskbar, x, width) {
                    log::warn!("taskbar refused the strip as a child; laid over it instead");
                }
            }
            *DOCKED_ON.lock() = Some(taskbar);
            let scale = scale_of(&sp);
            let _ = core.mutate(true, |d| {
                d.settings.float_docked = true;
                d.settings.float_dock_anchor = anchor.as_str().into();
                d.settings.float_dock_x = (x as f64 / scale).round() as i32;
                if let Some(p) = came_from {
                    d.settings.float_position = Some(p);
                }
                Ok(())
            });
            start_follower(app);
            log::info!("strip landed in the taskbar → {} at {x}, {width} px", anchor.as_str());
        }
        Landing::Float { rect } => {
            tb_dock::undock(hwnd);
            let _ = w.set_always_on_top(s.float_always_on_top);
            let _ = w.set_zoom(float_zoom(&s));
            let _ = w.set_size(PhysicalSize::new(rect.width().max(1) as u32, rect.height().max(1) as u32));
            let _ = w.set_position(PhysicalPosition::new(rect.left, rect.top));
            let _ = core.mutate(true, |d| {
                d.settings.float_docked = false;
                d.settings.float_position = Some((rect.left, rect.top));
                Ok(())
            });
            log::info!("strip landed floating at {},{} {}×{}", rect.left, rect.top, rect.width(), rect.height());
        }
    }
    finish_gesture();
}

fn finish_gesture() {
    DRAGGING.store(false, Ordering::SeqCst);
}

// ---- gestures ---------------------------------------------------------------------------

/// Every step of a move of the floating strip (window thread, inside the move loop).
pub fn on_float_moving(app: &AppHandle, x: i32, y: i32) {
    let Some(core) = app.try_state::<Arc<Core>>() else { return };
    let s = core.settings();
    let landing = if s.float_docked { None } else { LAYOUT.lock().and_then(|l| dock_landing_at(&s, &l, x, y)) };
    preview(&s, landing, false);
}

/// The move ended (`start`: where the strip was when it began): onto the ghost, if one showed.
pub fn on_float_dropped(app: &AppHandle, start: Rect) {
    let landing = LANDING.lock().take();
    ghost::hide();
    let Some(core) = app.try_state::<Arc<Core>>() else { return };
    if let Some(l @ Landing::Dock { .. }) = landing {
        land(app, core.inner(), l, Some((start.left, start.top)));
    }
}

/// The docked strip's handle was pressed: follow the mouse until it is let go, then land.
pub fn dock_drag(app: &AppHandle, core: &Arc<Core>) {
    let Some(hwnd) = app.get_webview_window("float").and_then(|w| w.hwnd().ok()).map(|h| h.0 as isize) else { return };
    let (Some(tb), Some((x0, width)), Some(c0), Some(l)) = (tb_dock::parent_taskbar(hwnd), tb_dock::docked_span(hwnd), tb_dock::cursor(), *LAYOUT.lock()) else {
        return;
    };
    let (Some(sp), Some(bar)) = (tb_dock::space(tb), tb_dock::taskbar_rect(tb)) else { return };
    if DRAGGING.swap(true, Ordering::SeqCst) {
        return;
    }
    let (app, core) = (app.clone(), core.clone());
    std::thread::spawn(move || {
        launcher_platform::win::com_init();
        let s = core.settings();
        let marks = fresh_marks(tb, &sp);
        let monitors = app.available_monitors().unwrap_or_default();
        let pull = (PULL_OUT * scale_of(&sp)).round() as i32;
        let mut landing = Landing::Dock { taskbar: tb, anchor: DockAnchor::parse(&s.float_dock_anchor), x: x0, width };
        let mut shown_x = x0;
        while launcher_platform::win::dock_input::primary_down() {
            std::thread::sleep(std::time::Duration::from_millis(8));
            let Some((cx, cy)) = tb_dock::cursor() else { continue };
            if cy < bar.top - pull || cy > bar.bottom + pull {
                if let Some(f) = float_landing_at(&monitors, &s, &l, cx, cy) {
                    landing = f;
                }
            } else {
                let (anchor, x) = snap_slot(x0 + (cx - c0.0), width, &marks, gap(&sp), snap_dist(&s, &sp));
                let x = exact_slot(anchor, x, &sp);
                landing = Landing::Dock { taskbar: tb, anchor, x, width };
                if x != shown_x {
                    tb_dock::place_docked(hwnd, x, width);
                    shown_x = x;
                }
            }
            preview(&s, Some(landing), true);
        }
        let app2 = app.clone();
        let _ = app.run_on_main_thread(move || {
            let core = app2.state::<Arc<Core>>();
            land(&app2, core.inner(), landing, None);
        });
    });
}

/// An end of the docked strip was pressed (`dir`: `West` / `East`): the same end drag as the
/// floating strip's (whole cells, the other end staying put, [`strip_sizing`]), followed here
/// because the taskbar keeps the window's own sizing loop from working. The count is saved as
/// `float_dock_slots` (floating keeps its own).
pub fn dock_resize(app: &AppHandle, core: &Arc<Core>, dir: &str) {
    use launcher_platform::snap::{drag_rect, strip_sizing, Strip};
    let edge = match dir {
        "West" => 1,
        "East" => 2,
        _ => return,
    };
    let Some(w) = app.get_webview_window("float") else { return };
    let Some(hwnd) = w.hwnd().ok().map(|h| h.0 as isize) else { return };
    let (Some(tb), Some((x0, w0)), Some(c0), Some(l)) = (tb_dock::parent_taskbar(hwnd), tb_dock::docked_span(hwnd), tb_dock::cursor(), *LAYOUT.lock()) else { return };
    let Some(sp) = tb_dock::space(tb) else { return };
    if DRAGGING.swap(true, Ordering::SeqCst) {
        return;
    }
    let core = core.clone();
    std::thread::spawn(move || {
        let s = core.settings();
        let fit = dock_fit(&l, &s, &sp);
        let strip = Strip { vertical: false, fixed: fit.fixed_px, cell: fit.cell_px, total: fit.max_n, min_across: 0, max_across: 0 };
        let start = Rect::new(x0, 0, x0 + w0, sp.height);
        let mut r = start;
        while launcher_platform::win::dock_input::primary_down() {
            std::thread::sleep(std::time::Duration::from_millis(8));
            let Some((cx, _)) = tb_dock::cursor() else { continue };
            let next = strip_sizing(drag_rect(start, edge, cx - c0.0, 0), start, edge, &strip);
            let next = Rect { left: next.left.max(0), ..next };
            if next != r {
                r = next;
                tb_dock::place_docked(hwnd, r.left, r.width());
            }
        }
        let n = ((r.width() as f64 - fit.fixed_px) / fit.cell_px).round().max(1.0) as usize;
        let scale = scale_of(&sp);
        let _ = core.mutate(true, |d| {
            // What would show anyway stays "automatic", so it keeps following the item count.
            d.settings.float_dock_slots = if n == fit.auto_n { 0 } else { n as u16 };
            if d.settings.float_dock_anchor == "free" {
                d.settings.float_dock_x = (r.left as f64 / scale).round() as i32;
            }
            Ok(())
        });
        log::info!("docked strip resized → {n} cells");
        finish_gesture();
    });
}

/// "Undock" from a menu: back to where it floated before (`float_position`).
pub fn undock(app: &AppHandle, core: &Arc<Core>) {
    let s = core.settings();
    let monitors = app.available_monitors().unwrap_or_default();
    let at = s.float_position.unwrap_or_else(|| tb_dock::cursor().unwrap_or((0, 0)));
    let rect = LAYOUT.lock().and_then(|l| float_rect(&monitors, &s, &l, at, at));
    match rect {
        Some(rect) => land(app, core, Landing::Float { rect }, None),
        None => {
            // No shape from the page yet: out of the taskbar, the next fit sizes it.
            if let Some(h) = app.get_webview_window("float").and_then(|w| w.hwnd().ok()) {
                tb_dock::undock(h.0 as isize);
            }
            let _ = core.mutate(true, |d| {
                d.settings.float_docked = false;
                Ok(())
            });
        }
    }
}

// ---- keeping it in place ----------------------------------------------------------------

/// Fits the docked strip to its layout: zoom and width (cells), holding the edge its anchor
/// keeps. Not docked yet (start-up, Explorer restarted): into the taskbar beside its anchor.
/// Never decides a new place for a strip that is already in the taskbar.
pub fn refit_docked(w: &WebviewWindow, core: &Arc<Core>, l: &FloatLayout, attempt: u32) {
    let Ok(h) = w.hwnd() else { return };
    let hwnd = h.0 as isize;
    if DRAGGING.load(Ordering::SeqCst) {
        return;
    }
    let s = core.settings();
    let inside = tb_dock::parent_taskbar(hwnd);
    let target = inside
        .or_else(|| DOCKED_ON.lock().filter(|t| tb_dock::space(*t).is_some()))
        .or_else(|| tb_dock::taskbar_near(s.float_position.map_or(0, |p| p.0)));
    let Some(tb) = target else {
        if attempt < 15 {
            let (w, core, l) = (w.clone(), core.clone(), *l);
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                refit_docked(&w, &core, &l, attempt + 1);
            });
        }
        return;
    };
    let Some(sp) = tb_dock::space(tb) else { return };
    if l.docked().across <= 0.0 {
        return;
    }
    let fit = dock_fit(l, &s, &sp);
    let _ = w.set_zoom(fit.k);
    let anchor = DockAnchor::parse(&s.float_dock_anchor);
    if inside.is_some() {
        if let Some((x, w0)) = tb_dock::docked_span(hwnd) {
            if w0 != fit.width {
                let x = if anchor.holds_right() { x + w0 - fit.width } else { x };
                tb_dock::place_docked(hwnd, x, fit.width);
            }
        }
    } else {
        let free = (s.float_dock_x as f64 * scale_of(&sp)).round() as i32;
        let x = slot_x(anchor, free, fit.width, &cached_marks(tb, &sp), gap(&sp));
        if !tb_dock::dock(hwnd, tb, x, fit.width) {
            log::warn!("taskbar refused the strip as a child; laid over it instead");
            return;
        }
        *DOCKED_ON.lock() = Some(tb);
        let app = w.app_handle().clone();
        std::thread::spawn(move || {
            launcher_platform::win::com_init();
            follow_once(&app);
        });
    }
    start_follower(w.app_handle());
}

/// Moves the docked strip beside its anchor if the taskbar changed under it (apps opened or
/// closed, tray icons, another app's docked window). Reads the taskbar through UI Automation:
/// call off the window thread.
fn follow_once(app: &AppHandle) {
    let Some(core) = app.try_state::<Arc<Core>>() else { return };
    let s = core.settings();
    if !s.float_docked || DRAGGING.load(Ordering::SeqCst) {
        return;
    }
    let Some(hwnd) = app.get_webview_window("float").and_then(|w| w.hwnd().ok()).map(|h| h.0 as isize) else { return };
    let (Some(tb), Some((x, width))) = (tb_dock::parent_taskbar(hwnd), tb_dock::docked_span(hwnd)) else { return };
    let Some(sp) = tb_dock::space(tb) else { return };
    let marks = fresh_marks(tb, &sp);
    let free = (s.float_dock_x as f64 * scale_of(&sp)).round() as i32;
    let want = slot_x(DockAnchor::parse(&s.float_dock_anchor), free, width, &marks, gap(&sp));
    if want != x && !DRAGGING.load(Ordering::SeqCst) {
        tb_dock::place_docked(hwnd, want, width);
    }
}

fn start_follower(app: &AppHandle) {
    static STARTED: AtomicBool = AtomicBool::new(false);
    if STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        launcher_platform::win::com_init();
        loop {
            std::thread::sleep(std::time::Duration::from_millis(1500));
            follow_once(&app);
        }
    });
}
