//! Finding, focusing and closing the top-level windows of processes.

use ::windows::core::BOOL;
use ::windows::Win32::Foundation::{HWND, LPARAM, RECT, WPARAM};
use ::windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, EnumWindows, GetWindow, GetWindowLongW, GetWindowPlacement, GetWindowRect, GetWindowTextLengthW,
    GetWindowThreadProcessId, IsIconic, IsWindowVisible, PostMessageW, SetForegroundWindow, SetWindowPlacement, ShowWindow,
    GWL_EXSTYLE, GW_OWNER, SW_HIDE, SW_RESTORE, WINDOWPLACEMENT, WM_CLOSE, WPF_RESTORETOMAXIMIZED, WS_EX_TOOLWINDOW,
};

/// Where Windows parks minimised windows.
const PARKED: i32 = -32000;

/// Placements saved by [`remember_placement`], by window handle.
static SAVED: std::sync::Mutex<Vec<(isize, WINDOWPLACEMENT)>> = std::sync::Mutex::new(Vec::new());

fn placement(h: HWND) -> Option<WINDOWPLACEMENT> {
    let mut wp = WINDOWPLACEMENT { length: std::mem::size_of::<WINDOWPLACEMENT>() as u32, ..Default::default() };
    unsafe { GetWindowPlacement(h, &mut wp).ok()? };
    Some(wp)
}

/// Hiding a minimised window (tao rewrites its style) drops the minimised state and the
/// remembered normal rectangle, leaving it parked at (-32000, -32000) with a caption-only size.
/// Call this just before hiding: while the window is still minimised, its placement still knows
/// where it belongs. [`unpark_hidden`] puts it back there.
pub fn remember_placement(hwnd: isize) {
    let h = HWND(hwnd as _);
    if !unsafe { IsIconic(h) }.as_bool() {
        return;
    }
    if let (Some(wp), Ok(mut saved)) = (placement(h), SAVED.lock()) {
        saved.retain(|(k, _)| *k != hwnd);
        saved.push((hwnd, wp));
    }
}

/// Moves a hidden window left parked at the minimised position back to the rectangle saved by
/// [`remember_placement`] (or, failing that, to wherever Windows still has it), still hidden.
/// Returns `Some(true)` when it was maximised before, so the caller can maximise it again
/// after showing it; `None` when nothing needed fixing.
pub fn unpark_hidden(hwnd: isize) -> Option<bool> {
    let h = HWND(hwnd as _);
    let saved = SAVED.lock().ok().and_then(|mut s| {
        let i = s.iter().position(|(k, _)| *k == hwnd)?;
        Some(s.remove(i).1)
    });
    unsafe {
        if IsIconic(h).as_bool() {
            return None;
        }
        let mut r = RECT::default();
        GetWindowRect(h, &mut r).ok()?;
        if r.left > PARKED || r.top > PARKED {
            return None;
        }
        let mut wp = saved.or_else(|| placement(h))?;
        let maximized = wp.flags.0 & WPF_RESTORETOMAXIMIZED.0 != 0;
        wp.showCmd = SW_HIDE.0 as u32;
        wp.flags.0 &= !WPF_RESTORETOMAXIMIZED.0;
        SetWindowPlacement(h, &wp).ok()?;
        Some(maximized)
    }
}

pub fn pid_of(hwnd: isize) -> u32 {
    let mut pid = 0u32;
    unsafe {
        GetWindowThreadProcessId(HWND(hwnd as _), Some(&mut pid));
    }
    pid
}

/// A window that gets a taskbar button: visible, unowned, not a tool window, titled.
pub fn is_app_window(hwnd: isize) -> bool {
    let h = HWND(hwnd as _);
    unsafe {
        if !IsWindowVisible(h).as_bool() {
            return false;
        }
        if GetWindow(h, GW_OWNER).map(|o| !o.is_invalid()).unwrap_or(false) {
            return false;
        }
        let ex = GetWindowLongW(h, GWL_EXSTYLE) as u32;
        if ex & WS_EX_TOOLWINDOW.0 != 0 {
            return false;
        }
        GetWindowTextLengthW(h) > 0
    }
}

/// `(hwnd, pid)` of every app window on the desktop.
pub fn app_windows() -> Vec<(isize, u32)> {
    unsafe extern "system" fn cb(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let out = &mut *(lparam.0 as *mut Vec<(isize, u32)>);
        let h = hwnd.0 as isize;
        if is_app_window(h) {
            out.push((h, pid_of(h)));
        }
        BOOL(1)
    }
    let mut out: Vec<(isize, u32)> = Vec::new();
    unsafe {
        let _ = EnumWindows(Some(cb), LPARAM(&mut out as *mut _ as isize));
    }
    out
}

pub fn windows_of(pids: &[u32]) -> Vec<isize> {
    app_windows().into_iter().filter(|(_, p)| pids.contains(p)).map(|(h, _)| h).collect()
}

pub fn focus(hwnd: isize) -> bool {
    let h = HWND(hwnd as _);
    unsafe {
        if IsIconic(h).as_bool() {
            let _ = ShowWindow(h, SW_RESTORE);
        }
        let _ = BringWindowToTop(h);
        SetForegroundWindow(h).as_bool()
    }
}

pub fn focus_any(pids: &[u32]) -> bool {
    windows_of(pids).first().map(|h| focus(*h)).unwrap_or(false)
}

pub fn close_all(pids: &[u32]) -> bool {
    let hs = windows_of(pids);
    for h in &hs {
        unsafe {
            let _ = PostMessageW(Some(HWND(*h as _)), WM_CLOSE, WPARAM(0), LPARAM(0));
        }
    }
    !hs.is_empty()
}
