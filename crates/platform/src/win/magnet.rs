//! Live magnet and proportional resizing for one small window (the floating launcher).
//!
//! The window is subclassed: while the user moves it (`WM_MOVING`) the proposed rectangle is
//! pulled onto nearby edges of the work areas and of other windows, and while they resize it
//! (`WM_SIZING`) [`strip_sizing`] decides what the drag means: a corner scales the strip as a
//! whole (ratio kept), an end adds or removes one cell at a time. Targets are collected once
//! when the move starts, not on every mouse move.

use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Mutex, OnceLock};

use windows::core::BOOL;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS};
use windows::Win32::Graphics::Gdi::{EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture;
use windows::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClassNameW, GetCursorPos, GetWindowRect, IsIconic, IsWindowVisible, PostMessageW, MINMAXINFO, WM_ENTERSIZEMOVE, WM_EXITSIZEMOVE,
    WM_GETMINMAXINFO, WM_MOVING, WM_NCLBUTTONDOWN, WM_SIZING,
};

use crate::snap::{drag_rect, snap, strip_sizing, Rect, Strip};

const SUBCLASS_ID: usize = 0x4D41_474E; // "MAGN"
/// Classes whose edges are the screen's anyway (desktop, taskbars) or never useful.
const SKIP_CLASSES: &[&str] = &["Progman", "WorkerW", "Shell_TrayWnd", "Shell_SecondaryTrayWnd"];

/// Snap distance in logical px (0 = magnet off).
static DIST: AtomicI32 = AtomicI32::new(12);
static TO_WINDOWS: AtomicBool = AtomicBool::new(true);
/// Between `WM_ENTERSIZEMOVE` and `WM_EXITSIZEMOVE`: the user holds the window.
static IN_SIZE_MOVE: AtomicBool = AtomicBool::new(false);
/// …and a frame (corner or edge) is being dragged, not the whole window.
static IN_SIZING: AtomicBool = AtomicBool::new(false);
/// Current shape of the strip, for resizing.
static STRIP: Mutex<Strip> = Mutex::new(Strip { vertical: true, fixed: 0.0, cell: 0.0, total: 0, min_across: 24, max_across: 0 });

/// Called (on the window's thread) when a move ends, with the cursor and the rectangle the move
/// started from: the launcher checks whether the strip was dropped on the taskbar.
type DropFn = Box<dyn Fn(i32, i32, Rect) + Send + Sync>;
static ON_DROP: OnceLock<DropFn> = OnceLock::new();

pub fn on_drop(f: impl Fn(i32, i32, Rect) + Send + Sync + 'static) {
    let _ = ON_DROP.set(Box::new(f));
}

/// Called (on the window's thread) at every step of a move, with the cursor: the launcher
/// previews where the strip would dock.
type MoveFn = Box<dyn Fn(i32, i32) + Send + Sync>;
static ON_MOVING: OnceLock<MoveFn> = OnceLock::new();

pub fn on_moving(f: impl Fn(i32, i32) + Send + Sync + 'static) {
    let _ = ON_MOVING.set(Box::new(f));
}

struct Drag {
    start: Rect,
    /// Cursor when the move / resize began; a resize follows how far it moved from here.
    cursor: POINT,
    sized: bool,
    areas: Vec<Rect>,
    windows: Vec<Rect>,
}

thread_local! {
    static DRAG: RefCell<Option<Drag>> = const { RefCell::new(None) };
}

/// Magnet settings: distance in logical px (`0` = off) and whether other windows count.
pub fn configure(dist: i32, to_windows: bool) {
    DIST.store(dist.max(0), Ordering::Relaxed);
    TO_WINDOWS.store(to_windows, Ordering::Relaxed);
}

/// The strip's current shape (physical px), used while the user resizes it.
pub fn set_strip(s: Strip) {
    if let Ok(mut g) = STRIP.lock() {
        *g = s;
    }
}

/// Whether the user is moving or resizing the window right now (button held).
pub fn in_size_move() -> bool {
    IN_SIZE_MOVE.load(Ordering::Relaxed)
}

/// Whether the user is dragging the window's frame right now. A size change during a plain
/// move (the strip re-laid out after leaving the taskbar) is not the user's: taking it for one
/// once saved the docked strip's 511 × 48 as the floating bar's scale and cells (2026-11-12).
pub fn in_user_resize() -> bool {
    IN_SIZE_MOVE.load(Ordering::Relaxed) && IN_SIZING.load(Ordering::Relaxed)
}

/// Subclasses `hwnd`. Safe to call again for the same window.
pub fn install(hwnd: isize) -> bool {
    unsafe { SetWindowSubclass(HWND(hwnd as _), Some(subclass), SUBCLASS_ID, 0).as_bool() }
}

/// Starts resizing from the cursor, as if that border of the window had been pressed
/// (`dir`: `North`, `NorthEast`, … `West`). Call on the window's thread.
///
/// Replaces tao's `drag_resize_window`, which posts `WM_NCLBUTTONDOWN` with a *pointer* to the
/// cursor position as `lParam` instead of the packed coordinates: Windows then starts the
/// sizing loop from a meaningless point, and the window and the cursor jump on the first click
/// (seen 2026-11-12 on the floating bar's corners).
pub fn begin_resize(hwnd: isize, dir: &str) -> bool {
    // HTLEFT … HTBOTTOMRIGHT
    let ht: usize = match dir {
        "West" => 10,
        "East" => 11,
        "North" => 12,
        "NorthWest" => 13,
        "NorthEast" => 14,
        "South" => 15,
        "SouthWest" => 16,
        "SouthEast" => 17,
        _ => return false,
    };
    press_frame(hwnd, ht)
}

/// Starts moving the window with the held mouse button, as if its caption had been pressed.
pub fn begin_move(hwnd: isize) -> bool {
    press_frame(hwnd, 2) // HTCAPTION
}

/// Posts `WM_NCLBUTTONDOWN` for hit-test `ht` at the cursor, so Windows runs its own move /
/// size loop from there.
fn press_frame(hwnd: isize, ht: usize) -> bool {
    let mut p = POINT::default();
    if unsafe { GetCursorPos(&mut p) }.is_err() {
        return false;
    }
    // MAKELPARAM(x, y) with signed 16-bit screen coordinates (left / upper monitors are negative).
    let lp = (((p.y as i16 as u16 as u32) << 16) | p.x as i16 as u16 as u32) as isize;
    unsafe {
        let _ = ReleaseCapture();
        PostMessageW(Some(HWND(hwnd as _)), WM_NCLBUTTONDOWN, WPARAM(ht), LPARAM(lp)).is_ok()
    }
}

fn rect(r: RECT) -> Rect {
    Rect::new(r.left, r.top, r.right, r.bottom)
}

fn win_rect(r: Rect) -> RECT {
    RECT { left: r.left, top: r.top, right: r.right, bottom: r.bottom }
}

unsafe extern "system" fn subclass(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM, _id: usize, _data: usize) -> LRESULT {
    match msg {
        WM_ENTERSIZEMOVE => {
            IN_SIZE_MOVE.store(true, Ordering::Relaxed);
            IN_SIZING.store(false, Ordering::Relaxed);
            let mut r = RECT::default();
            let _ = unsafe { GetWindowRect(hwnd, &mut r) };
            let windows = if TO_WINDOWS.load(Ordering::Relaxed) { other_windows(hwnd) } else { Vec::new() };
            let mut cursor = POINT::default();
            let _ = unsafe { GetCursorPos(&mut cursor) };
            DRAG.with(|d| *d.borrow_mut() = Some(Drag { start: rect(r), cursor, sized: false, areas: work_areas(), windows }));
        }
        WM_EXITSIZEMOVE => {
            IN_SIZE_MOVE.store(false, Ordering::Relaxed);
            IN_SIZING.store(false, Ordering::Relaxed);
            let moved = DRAG.with(|d| d.borrow_mut().take()).filter(|d| !d.sized);
            if let (Some(d), Some(cb)) = (moved, ON_DROP.get()) {
                let mut p = POINT::default();
                if unsafe { GetCursorPos(&mut p) }.is_ok() {
                    cb(p.x, p.y, d.start);
                }
            }
        }
        WM_MOVING if lp.0 != 0 => {
            let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96) as i32;
            let dist = DIST.load(Ordering::Relaxed) * dpi / 96;
            let done = DRAG.with(|d| {
                let d = d.borrow();
                let Some(d) = d.as_ref() else { return false };
                let r = unsafe { &mut *(lp.0 as *mut RECT) };
                *r = win_rect(snap(rect(*r), &d.areas, &d.windows, dist));
                true
            });
            if let Some(cb) = ON_MOVING.get() {
                let mut p = POINT::default();
                if unsafe { GetCursorPos(&mut p) }.is_ok() {
                    cb(p.x, p.y);
                }
            }
            if done {
                return LRESULT(1);
            }
        }
        WM_SIZING if lp.0 != 0 => {
            IN_SIZING.store(true, Ordering::Relaxed);
            let done = DRAG.with(|d| {
                let mut d = d.borrow_mut();
                let Some(d) = d.as_mut() else { return false };
                d.sized = true;
                let r = unsafe { &mut *(lp.0 as *mut RECT) };
                let strip = STRIP.lock().map(|g| *g).unwrap_or_default();
                // From the cursor, not from `*r`: Windows has already stretched that to its
                // minimum tracking size (see `drag_rect`).
                let mut p = POINT::default();
                let want = if unsafe { GetCursorPos(&mut p) }.is_ok() {
                    drag_rect(d.start, wp.0 as u32, p.x - d.cursor.x, p.y - d.cursor.y)
                } else {
                    rect(*r)
                };
                *r = win_rect(strip_sizing(want, d.start, wp.0 as u32, &strip));
                true
            });
            if done {
                return LRESULT(1);
            }
        }
        // A resizable window would otherwise be held at the system's minimum tracking size
        // (wider than the strip); let it be as small as its content.
        WM_GETMINMAXINFO if lp.0 != 0 => {
            let res = unsafe { DefSubclassProc(hwnd, msg, wp, lp) };
            let info = unsafe { &mut *(lp.0 as *mut MINMAXINFO) };
            info.ptMinTrackSize.x = 8;
            info.ptMinTrackSize.y = 8;
            return res;
        }
        _ => {}
    }
    unsafe { DefSubclassProc(hwnd, msg, wp, lp) }
}

pub fn work_areas() -> Vec<Rect> {
    unsafe extern "system" fn each(m: HMONITOR, _: HDC, _: *mut RECT, data: LPARAM) -> BOOL {
        let out = unsafe { &mut *(data.0 as *mut Vec<Rect>) };
        let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        if unsafe { GetMonitorInfoW(m, &mut info) }.as_bool() {
            out.push(rect(info.rcWork));
        }
        true.into()
    }
    let mut out: Vec<Rect> = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(None, None, Some(each), LPARAM(&mut out as *mut _ as isize));
    }
    out
}

/// Visible, uncloaked top-level windows other than `me`, by their visible frame (without the
/// invisible resize border Windows 10/11 adds around normal windows).
fn other_windows(me: HWND) -> Vec<Rect> {
    struct Ctx {
        me: HWND,
        out: Vec<Rect>,
    }
    unsafe extern "system" fn each(h: HWND, data: LPARAM) -> BOOL {
        let ctx = unsafe { &mut *(data.0 as *mut Ctx) };
        if h == ctx.me || !unsafe { IsWindowVisible(h) }.as_bool() || unsafe { IsIconic(h) }.as_bool() {
            return true.into();
        }
        let mut cloaked = 0u32;
        let _ = unsafe { DwmGetWindowAttribute(h, DWMWA_CLOAKED, &mut cloaked as *mut _ as *mut _, 4) };
        if cloaked != 0 {
            return true.into();
        }
        let mut buf = [0u16; 64];
        let n = unsafe { GetClassNameW(h, &mut buf) } as usize;
        let class = String::from_utf16_lossy(&buf[..n.min(buf.len())]);
        if SKIP_CLASSES.contains(&class.as_str()) {
            return true.into();
        }
        let mut r = RECT::default();
        let framed = unsafe { DwmGetWindowAttribute(h, DWMWA_EXTENDED_FRAME_BOUNDS, &mut r as *mut _ as *mut _, std::mem::size_of::<RECT>() as u32) };
        if framed.is_err() && unsafe { GetWindowRect(h, &mut r) }.is_err() {
            return true.into();
        }
        let r = rect(r);
        if r.width() >= 40 && r.height() >= 40 {
            ctx.out.push(r);
        }
        true.into()
    }
    let mut ctx = Ctx { me, out: Vec::new() };
    unsafe {
        let _ = EnumWindows(Some(each), LPARAM(&mut ctx as *mut _ as isize));
    }
    ctx.out
}
