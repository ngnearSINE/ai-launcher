//! Notifies when any top-level window appears, so instance windows can be given their
//! taskbar identity the moment they show up (no polling needed for that part).

use std::sync::OnceLock;

use ::windows::Win32::Foundation::HWND;
use ::windows::Win32::UI::Accessibility::{SetWinEventHook, HWINEVENTHOOK};
use ::windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetAncestor, GetMessageW, TranslateMessage, EVENT_OBJECT_SHOW, GA_ROOT, MSG, OBJID_WINDOW,
    WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS,
};

type Handler = Box<dyn Fn(isize, u32) + Send + Sync>;
static HANDLER: OnceLock<Handler> = OnceLock::new();

unsafe extern "system" fn on_event(
    _hook: HWINEVENTHOOK,
    _event: u32,
    hwnd: HWND,
    id_object: i32,
    id_child: i32,
    _thread: u32,
    _time: u32,
) {
    if id_object != OBJID_WINDOW.0 || id_child != 0 || hwnd.is_invalid() {
        return;
    }
    if GetAncestor(hwnd, GA_ROOT) != hwnd {
        return;
    }
    let h = hwnd.0 as isize;
    if let Some(handler) = HANDLER.get() {
        handler(h, super::windows::pid_of(h));
    }
}

/// Starts the hook on its own thread (once per process). `handler(hwnd, pid)` runs on that
/// thread and must return quickly.
pub fn start(handler: impl Fn(isize, u32) + Send + Sync + 'static) {
    if HANDLER.set(Box::new(handler)).is_err() {
        return;
    }
    std::thread::Builder::new()
        .name("window-watcher".into())
        .spawn(|| unsafe {
            let _hook = SetWinEventHook(
                EVENT_OBJECT_SHOW,
                EVENT_OBJECT_SHOW,
                None,
                Some(on_event),
                0,
                0,
                WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
            );
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        })
        .expect("spawn window watcher");
}
