//! Mouse input for a window docked into the taskbar.
//!
//! Windows 11 draws the taskbar with XAML and routes the pointer to it by composition, not by
//! the window tree: a child window placed inside `Shell_TrayWnd` is shown (and `WindowFromPoint`
//! finds it) but real mouse input never reaches it — only messages posted to it do (checked
//! 2026-11-12; AI Usage works around the same thing by polling the button for its one click).
//!
//! While a window is docked, a low-level mouse hook watches the cursor and, over that window
//! only, posts the same mouse messages to the web view inside it, so hover, clicks, the context
//! menu and the wheel behave as anywhere else. Rules that keep other apps unaffected:
//! - nothing outside the docked window's rectangle is touched (a press that started on it is
//!   followed until its release, like a capture);
//! - the left button is never swallowed: the system's button state stays true for everyone
//!   (other docked widgets that poll it, Windows' own move loop when the strip is pulled out);
//! - the right button and the wheel over the window are swallowed, so the taskbar underneath
//!   does not also open its own menu.

use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU32, Ordering};

use windows::core::{BOOL, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::ScreenToClient;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, EnumChildWindows, GetMessageW, GetWindowRect, IsWindow, IsWindowVisible, PostMessageW, PostThreadMessageW, SetWindowsHookExW,
    TranslateMessage, UnhookWindowsHookEx, HC_ACTION, MSG, MSLLHOOKSTRUCT, WH_MOUSE_LL, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN,
    WM_MBUTTONUP, WM_MOUSEHWHEEL, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_QUIT, WM_RBUTTONDOWN, WM_RBUTTONUP,
};

const WM_MOUSELEAVE: u32 = 0x02A3;
const MK_LBUTTON: usize = 0x0001;
const MK_RBUTTON: usize = 0x0002;
const MK_MBUTTON: usize = 0x0010;

/// The docked window (0 = none).
static HOST: AtomicIsize = AtomicIsize::new(0);
/// The web view's input window inside it (found on demand, re-found when it goes away).
static TARGET: AtomicIsize = AtomicIsize::new(0);
/// Hook thread id (0 = not running).
static THREAD: AtomicU32 = AtomicU32::new(0);
/// Buttons held as seen by the hook (`MK_*` bits), and whether a press started on the host.
static BUTTONS: AtomicIsize = AtomicIsize::new(0);
static CAPTURED: AtomicBool = AtomicBool::new(false);
static INSIDE: AtomicBool = AtomicBool::new(false);

/// Starts (or retargets) forwarding for `host`, a window docked into the taskbar.
pub fn start(host: isize) {
    HOST.store(host, Ordering::SeqCst);
    TARGET.store(0, Ordering::SeqCst);
    if THREAD.load(Ordering::SeqCst) != 0 {
        return;
    }
    std::thread::spawn(|| unsafe {
        let module = GetModuleHandleW(PCWSTR::null()).ok().map(|m| m.into());
        let Ok(hook) = SetWindowsHookExW(WH_MOUSE_LL, Some(proc), module, 0) else {
            log::warn!("docked input: mouse hook refused");
            return;
        };
        THREAD.store(GetCurrentThreadId(), Ordering::SeqCst);
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        let _ = UnhookWindowsHookEx(hook);
        THREAD.store(0, Ordering::SeqCst);
    });
}

/// Stops forwarding (the window left the taskbar).
pub fn stop() {
    HOST.store(0, Ordering::SeqCst);
    TARGET.store(0, Ordering::SeqCst);
    CAPTURED.store(false, Ordering::SeqCst);
    INSIDE.store(false, Ordering::SeqCst);
    let id = THREAD.load(Ordering::SeqCst);
    if id != 0 {
        let _ = unsafe { PostThreadMessageW(id, WM_QUIT, WPARAM(0), LPARAM(0)) };
    }
}

/// The deepest `Chrome_RenderWidgetHostHWND` under `host` (where a web view takes its input),
/// or `host` itself when there is none.
fn target(host: HWND) -> HWND {
    let cached = HWND(TARGET.load(Ordering::SeqCst) as _);
    if !cached.0.is_null() && unsafe { IsWindow(Some(cached)) }.as_bool() {
        return cached;
    }
    unsafe extern "system" fn each(h: HWND, out: LPARAM) -> BOOL {
        if super::dock::class_of(h) == "Chrome_RenderWidgetHostHWND" {
            *(out.0 as *mut isize) = h.0 as isize;
            return false.into();
        }
        true.into()
    }
    let mut found: isize = 0;
    unsafe {
        let _ = EnumChildWindows(Some(host), Some(each), LPARAM(&mut found as *mut isize as isize));
    }
    let h = if found != 0 { HWND(found as _) } else { host };
    TARGET.store(h.0 as isize, Ordering::SeqCst);
    h
}

fn contains(r: &RECT, p: POINT) -> bool {
    p.x >= r.left && p.x < r.right && p.y >= r.top && p.y < r.bottom
}

fn client_lparam(to: HWND, p: POINT) -> LPARAM {
    let mut c = p;
    unsafe {
        let _ = ScreenToClient(to, &mut c);
    }
    LPARAM((((c.y as u16 as u32) << 16) | c.x as u16 as u32) as isize)
}

unsafe extern "system" fn proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        if let Some(swallow) = handle(wparam.0 as u32, &*(lparam.0 as *const MSLLHOOKSTRUCT)) {
            if swallow {
                return LRESULT(1);
            }
        }
    }
    CallNextHookEx(None, code, wparam, lparam)
}

/// Forwards one mouse event when it concerns the host; `Some(true)` = keep it from the taskbar.
fn handle(msg: u32, info: &MSLLHOOKSTRUCT) -> Option<bool> {
    // Button state first, for every event anywhere (it is what a forwarded message reports).
    let bit = match msg {
        WM_LBUTTONDOWN | WM_LBUTTONUP => MK_LBUTTON,
        WM_RBUTTONDOWN | WM_RBUTTONUP => MK_RBUTTON,
        WM_MBUTTONDOWN | WM_MBUTTONUP => MK_MBUTTON,
        _ => 0,
    } as isize;
    let down = matches!(msg, WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN);
    let up = matches!(msg, WM_LBUTTONUP | WM_RBUTTONUP | WM_MBUTTONUP);
    let before = BUTTONS.load(Ordering::SeqCst);
    let held = if down { before | bit } else if up { before & !bit } else { before };
    BUTTONS.store(held, Ordering::SeqCst);

    let host = HWND(HOST.load(Ordering::SeqCst) as _);
    if host.0.is_null() {
        return None;
    }
    let mut r = RECT::default();
    if !unsafe { IsWindowVisible(host) }.as_bool() || unsafe { GetWindowRect(host, &mut r) }.is_err() {
        return None;
    }
    let p = info.pt;
    let inside = contains(&r, p);
    let captured = CAPTURED.load(Ordering::SeqCst);
    let to = target(host);
    let post = |m: u32, w: usize, l: LPARAM| unsafe {
        let _ = PostMessageW(Some(to), m, WPARAM(w), l);
    };

    if !inside && !captured {
        if INSIDE.swap(false, Ordering::SeqCst) {
            post(WM_MOUSELEAVE, 0, LPARAM(0));
        }
        return None;
    }
    INSIDE.store(inside, Ordering::SeqCst);
    match msg {
        WM_MOUSEMOVE => {
            post(WM_MOUSEMOVE, held as usize, client_lparam(to, p));
            Some(false)
        }
        WM_MOUSEWHEEL | WM_MOUSEHWHEEL => {
            // Wheel messages carry screen coordinates and the delta in the high word.
            let w = (info.mouseData & 0xFFFF_0000) as usize | held as usize;
            post(msg, w, LPARAM((((p.y as u16 as u32) << 16) | p.x as u16 as u32) as isize));
            Some(inside)
        }
        _ if down || up => {
            if down && inside {
                CAPTURED.store(true, Ordering::SeqCst);
            }
            if up && held == 0 {
                CAPTURED.store(false, Ordering::SeqCst);
            }
            post(msg, held as usize, client_lparam(to, p));
            // The left button stays with the system; the others are ours when pressed on the
            // strip (a release is kept back only if its press was).
            Some(bit != MK_LBUTTON as isize && ((down && inside) || (up && captured)))
        }
        _ => None,
    }
}
