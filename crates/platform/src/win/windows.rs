//! Finding, focusing and closing the top-level windows of processes.

use ::windows::core::BOOL;
use ::windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use ::windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, EnumWindows, GetWindow, GetWindowLongW, GetWindowTextLengthW, GetWindowThreadProcessId, IsIconic,
    IsWindowVisible, PostMessageW, SetForegroundWindow, ShowWindow, GWL_EXSTYLE, GW_OWNER, SW_RESTORE, WM_CLOSE,
    WS_EX_TOOLWINDOW,
};

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
