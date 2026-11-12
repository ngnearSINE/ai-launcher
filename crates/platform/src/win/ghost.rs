//! A translucent, click-through outline that shows where a window will land before it is let
//! go (the floating strip on its way into or out of the taskbar). One plain Win32 window on its
//! own thread, so it can be moved from any thread without waiting on the launcher's windows.

use std::sync::atomic::{AtomicIsize, AtomicU32, Ordering};
use std::sync::Mutex;

use windows::core::w;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreatePen, CreateSolidBrush, DeleteObject, EndPaint, InvalidateRect, RoundRect, SelectObject, HGDIOBJ, PAINTSTRUCT, PS_SOLID,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetClientRect, GetMessageW, RegisterClassW, SetLayeredWindowAttributes,
    SetWindowPos, ShowWindowAsync, TranslateMessage, HTTRANSPARENT, HWND_TOPMOST, LWA_ALPHA, MSG, SWP_ASYNCWINDOWPOS, SWP_NOACTIVATE, SWP_SHOWWINDOW,
    SW_HIDE, WM_ERASEBKGND, WM_NCHITTEST, WM_PAINT, WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT,
    WS_POPUP,
};

static HWND_GHOST: AtomicIsize = AtomicIsize::new(0);
/// Accent colour as 0x00BBGGRR.
static COLOR: AtomicU32 = AtomicU32::new(0x00FF_840A);
static STARTING: Mutex<bool> = Mutex::new(false);

/// Shows the outline over `(left, top, right, bottom)` (physical screen px) in `rgb`
/// (0xRRGGBB). Cheap to call on every mouse step.
pub fn show(r: (i32, i32, i32, i32), rgb: u32) {
    let bgr = ((rgb & 0xFF) << 16) | (rgb & 0xFF00) | ((rgb >> 16) & 0xFF);
    let changed = COLOR.swap(bgr, Ordering::Relaxed) != bgr;
    let Some(h) = window() else { return };
    unsafe {
        if changed {
            let _ = InvalidateRect(Some(h), None, true);
        }
        let _ = SetWindowPos(
            h,
            Some(HWND_TOPMOST),
            r.0,
            r.1,
            (r.2 - r.0).max(1),
            (r.3 - r.1).max(1),
            SWP_NOACTIVATE | SWP_SHOWWINDOW | SWP_ASYNCWINDOWPOS,
        );
    }
}

/// Hides the outline (no-op when it is not shown).
pub fn hide() {
    let h = HWND_GHOST.load(Ordering::SeqCst);
    if h != 0 {
        let _ = unsafe { ShowWindowAsync(HWND(h as _), SW_HIDE) };
    }
}

/// The outline window, created on first use on its own thread.
fn window() -> Option<HWND> {
    let h = HWND_GHOST.load(Ordering::SeqCst);
    if h != 0 {
        return Some(HWND(h as _));
    }
    let mut starting = STARTING.lock().ok()?;
    if !*starting {
        *starting = true;
        std::thread::spawn(run);
    }
    drop(starting);
    // The first preview waits briefly for the window; later ones never do.
    for _ in 0..50 {
        std::thread::sleep(std::time::Duration::from_millis(2));
        let h = HWND_GHOST.load(Ordering::SeqCst);
        if h != 0 {
            return Some(HWND(h as _));
        }
    }
    None
}

fn run() {
    unsafe {
        let Ok(module) = GetModuleHandleW(None) else { return };
        let class = WNDCLASSW { lpfnWndProc: Some(proc), hInstance: module.into(), lpszClassName: w!("AiLauncherGhost"), ..Default::default() };
        RegisterClassW(&class);
        let Ok(h) = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TOPMOST,
            w!("AiLauncherGhost"),
            w!(""),
            WS_POPUP,
            0,
            0,
            1,
            1,
            None,
            None,
            Some(module.into()),
            None,
        ) else {
            log::warn!("ghost window could not be created");
            return;
        };
        let _ = SetLayeredWindowAttributes(h, COLORREF(0), 150, LWA_ALPHA);
        let round = DWMWCP_ROUND;
        let _ = DwmSetWindowAttribute(h, DWMWA_WINDOW_CORNER_PREFERENCE, &round as *const _ as _, std::mem::size_of_val(&round) as u32);
        HWND_GHOST.store(h.0 as isize, Ordering::SeqCst);
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

unsafe extern "system" fn proc(h: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_NCHITTEST => LRESULT(HTTRANSPARENT as isize),
        WM_ERASEBKGND => LRESULT(1),
        WM_PAINT => {
            // A soft fill with a brighter rim, in the accent colour.
            let mut ps = PAINTSTRUCT::default();
            let dc = BeginPaint(h, &mut ps);
            let mut r = RECT::default();
            let _ = GetClientRect(h, &mut r);
            let c = COLOR.load(Ordering::Relaxed);
            let fill = CreateSolidBrush(COLORREF(blend(c, 0x0020_2020)));
            let pen = CreatePen(PS_SOLID, 3, COLORREF(c));
            let old_brush = SelectObject(dc, HGDIOBJ(fill.0));
            let old_pen = SelectObject(dc, HGDIOBJ(pen.0));
            let _ = RoundRect(dc, r.left + 1, r.top + 1, r.right - 1, r.bottom - 1, 16, 16);
            SelectObject(dc, old_brush);
            SelectObject(dc, old_pen);
            let _ = DeleteObject(HGDIOBJ(fill.0));
            let _ = DeleteObject(HGDIOBJ(pen.0));
            let _ = EndPaint(h, &ps);
            LRESULT(0)
        }
        _ => DefWindowProcW(h, msg, wp, lp),
    }
}

/// `c` darkened halfway toward `toward` (both 0x00BBGGRR).
fn blend(c: u32, toward: u32) -> u32 {
    let ch = |v: u32, s: u32| (((v >> s) & 0xFF) + ((toward >> s) & 0xFF)) / 2 << s;
    ch(c, 0) | ch(c, 8) | ch(c, 16)
}

