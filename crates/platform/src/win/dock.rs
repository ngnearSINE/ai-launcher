//! Docking a small window into the Windows taskbar (as AI Usage does): the window becomes a
//! child of `Shell_TrayWnd` / `Shell_SecondaryTrayWnd` and sits just left of the notification
//! area, as tall as the taskbar. If Windows refuses the re-parenting, it is laid over the taskbar
//! as a top-most pop-up instead.

use windows::core::{BOOL, PCWSTR};
use windows::Win32::Foundation::{HANDLE, HWND, LPARAM, POINT, RECT};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, FindWindowExW, GetClassNameW, GetClientRect, GetParent, GetPropW, GetWindowLongW, GetWindowRect, IsWindow, RemovePropW, SetParent,
    SetPropW, SetWindowLongW, SetWindowPos, GWL_EXSTYLE, GWL_STYLE, HWND_TOPMOST, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    SWP_NOZORDER, SWP_SHOWWINDOW, WS_CHILD, WS_EX_APPWINDOW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_POPUP,
};
use windows::Win32::Graphics::Gdi::{ClientToScreen, ScreenToClient};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};
use windows::Win32::System::Variant::VARIANT;
use windows::Win32::UI::Accessibility::{CUIAutomation, IUIAutomation, TreeScope_Children, TreeScope_Descendants, UIA_AutomationIdPropertyId};
use windows::Win32::UI::WindowsAndMessaging::{GetCursorPos, SM_SWAPBUTTON};

use crate::dock_slot::DockMarks;
use crate::snap::Rect;

const PROP: &str = "AiLauncher.OriginalStyle";
/// Caption, frame and system-menu bits; left on a child they eat into its client area.
const FRAME_BITS: i32 = 0x00CF_0000;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub(crate) fn class_of(h: HWND) -> String {
    let mut buf = [0u16; 64];
    let n = unsafe { GetClassNameW(h, &mut buf) } as usize;
    String::from_utf16_lossy(&buf[..n.min(buf.len())])
}

fn rect_of(h: HWND) -> Option<Rect> {
    let mut r = RECT::default();
    unsafe { GetWindowRect(h, &mut r) }.ok()?;
    Some(Rect::new(r.left, r.top, r.right, r.bottom))
}

/// Every taskbar (main and secondary monitors), with its screen rectangle.
pub fn taskbars() -> Vec<(isize, Rect)> {
    unsafe extern "system" fn each(h: HWND, data: LPARAM) -> BOOL {
        let out = unsafe { &mut *(data.0 as *mut Vec<(isize, Rect)>) };
        let c = class_of(h);
        if c == "Shell_TrayWnd" || c == "Shell_SecondaryTrayWnd" {
            if let Some(r) = rect_of(h) {
                out.push((h.0 as isize, r));
            }
        }
        true.into()
    }
    let mut out: Vec<(isize, Rect)> = Vec::new();
    unsafe {
        let _ = EnumWindows(Some(each), LPARAM(&mut out as *mut _ as isize));
    }
    out
}

/// The horizontal taskbar under a screen point, if any.
pub fn taskbar_at(x: i32, y: i32) -> Option<isize> {
    taskbars()
        .into_iter()
        .find(|(_, r)| r.width() > r.height() && x >= r.left && x < r.right && y >= r.top && y < r.bottom)
        .map(|(h, _)| h)
}

/// The horizontal taskbar on the monitor containing screen x, else the main one.
pub fn taskbar_near(x: i32) -> Option<isize> {
    let all: Vec<(isize, Rect)> = taskbars().into_iter().filter(|(_, r)| r.width() > r.height()).collect();
    all.iter()
        .find(|(_, r)| x >= r.left && x < r.right)
        .or_else(|| all.iter().find(|(h, _)| class_of(HWND(*h as _)) == "Shell_TrayWnd"))
        .or(all.first())
        .map(|(h, _)| *h)
}

#[derive(Debug, Clone, Copy)]
pub struct TaskbarSpace {
    /// Client size of the taskbar (physical px).
    pub width: i32,
    pub height: i32,
    /// Client x where the notification area (tray icons, clock) starts.
    pub right: i32,
    pub dpi: u32,
}

/// Room inside a taskbar: its client size and where the notification area begins.
pub fn space(taskbar: isize) -> Option<TaskbarSpace> {
    let tb = HWND(taskbar as _);
    let mut c = RECT::default();
    unsafe { GetClientRect(tb, &mut c) }.ok()?;
    let (width, height) = (c.right - c.left, c.bottom - c.top);
    let mut right = width;
    let tray_class = wide("TrayNotifyWnd");
    if let Ok(tray) = unsafe { FindWindowExW(Some(tb), None, PCWSTR(tray_class.as_ptr()), PCWSTR::null()) } {
        if let Some(r) = rect_of(tray) {
            let mut p = POINT { x: r.left, y: r.top };
            if unsafe { ScreenToClient(tb, &mut p) }.as_bool() && p.x > 0 && p.x <= width {
                right = p.x;
            }
        }
    }
    let dpi = unsafe { GetDpiForWindow(tb) }.max(96);
    Some(TaskbarSpace { width, height, right, dpi })
}

/// Whether `hwnd` currently sits inside a taskbar.
pub fn is_docked(hwnd: isize) -> bool {
    let p = unsafe { GetParent(HWND(hwnd as _)) }.unwrap_or_default();
    !p.0.is_null() && {
        let c = class_of(p);
        c == "Shell_TrayWnd" || c == "Shell_SecondaryTrayWnd"
    }
}

/// Makes `hwnd` a child of `taskbar` at client `x`, `width` px wide and as tall as the taskbar.
/// Returns `false` when Windows refused the child and it was laid over the taskbar instead.
pub fn dock(hwnd: isize, taskbar: isize, x: i32, width: i32) -> bool {
    let (h, tb) = (HWND(hwnd as _), HWND(taskbar as _));
    let Some(sp) = space(taskbar) else { return false };
    unsafe {
        let mut style = GetWindowLongW(h, GWL_STYLE);
        if style & WS_CHILD.0 as i32 == 0 {
            let name = wide(PROP);
            let _ = SetPropW(h, PCWSTR(name.as_ptr()), Some(HANDLE(style as isize as _)));
        }
        style = (style & !(WS_POPUP.0 as i32) & !FRAME_BITS) | WS_CHILD.0 as i32;
        SetWindowLongW(h, GWL_STYLE, style);
        let ex = (GetWindowLongW(h, GWL_EXSTYLE) | WS_EX_TOOLWINDOW.0 as i32) & !(WS_EX_APPWINDOW.0 as i32) & !(WS_EX_NOACTIVATE.0 as i32);
        SetWindowLongW(h, GWL_EXSTYLE, ex);
        let _ = SetParent(h, Some(tb));
        let parented = GetParent(h).map(|p| p == tb).unwrap_or(false);
        let x = x.clamp(0, (sp.right - width).max(0));
        if parented {
            let _ = SetWindowPos(h, None, x, 0, width, sp.height, SWP_NOACTIVATE | SWP_FRAMECHANGED | SWP_SHOWWINDOW);
            // The taskbar keeps real mouse input from its children: hand it over ourselves.
            super::dock_input::start(hwnd);
            return true;
        }
        // Refused: stay a pop-up over the taskbar.
        let style = (style & !(WS_CHILD.0 as i32)) | WS_POPUP.0 as i32;
        SetWindowLongW(h, GWL_STYLE, style);
        let r = rect_of(tb).unwrap_or_default();
        let _ = SetWindowPos(h, Some(HWND_TOPMOST), r.left + x, r.top, width, sp.height, SWP_NOACTIVATE | SWP_FRAMECHANGED | SWP_SHOWWINDOW);
        false
    }
}

/// Moves / resizes a docked window inside its taskbar (client coordinates).
pub fn place_docked(hwnd: isize, x: i32, width: i32) -> bool {
    // Only while it really is in a taskbar. A late placement (one queued just before the strip
    // was pulled out) once took the floating window's hidden owner for a taskbar and parked the
    // strip at the top of the screen, 511 × 48 (2026-11-12).
    if !is_docked(hwnd) {
        return false;
    }
    let h = HWND(hwnd as _);
    let Some(tb) = unsafe { GetParent(h) }.ok() else { return false };
    let Some(sp) = space(tb.0 as isize) else { return false };
    let x = x.clamp(0, (sp.right - width).max(0));
    unsafe { SetWindowPos(h, None, x, 0, width, sp.height, SWP_NOACTIVATE | SWP_NOZORDER) }.is_ok()
}

/// Back to an ordinary top-level window with the style it had before docking.
pub fn undock(hwnd: isize) {
    let h = HWND(hwnd as _);
    if !unsafe { IsWindow(Some(h)) }.as_bool() {
        return;
    }
    super::dock_input::stop();
    unsafe {
        let _ = SetParent(h, None);
        let name = wide(PROP);
        let saved = GetPropW(h, PCWSTR(name.as_ptr()));
        let style = if !saved.0.is_null() {
            saved.0 as isize as i32
        } else {
            (GetWindowLongW(h, GWL_STYLE) & !(WS_CHILD.0 as i32)) | WS_POPUP.0 as i32
        };
        let _ = RemovePropW(h, PCWSTR(name.as_ptr()));
        SetWindowLongW(h, GWL_STYLE, style);
        let _ = SetWindowPos(h, Some(HWND_TOPMOST), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED | SWP_SHOWWINDOW);
    }
}

// ---- moving inside the taskbar ------------------------------------------------------------

/// The taskbar a docked window sits in.
pub fn parent_taskbar(hwnd: isize) -> Option<isize> {
    is_docked(hwnd).then(|| unsafe { GetParent(HWND(hwnd as _)) }.ok()).flatten().map(|p| p.0 as isize)
}

/// A docked window's left edge in its taskbar's client x, and its width (physical px).
pub fn docked_span(hwnd: isize) -> Option<(i32, i32)> {
    let h = HWND(hwnd as _);
    let tb = unsafe { GetParent(h) }.ok()?;
    let r = rect_of(h)?;
    let mut p = POINT { x: r.left, y: r.top };
    unsafe { ScreenToClient(tb, &mut p) }.as_bool().then_some((p.x, r.width()))
}

/// A taskbar's screen rectangle.
pub fn taskbar_rect(taskbar: isize) -> Option<Rect> {
    rect_of(HWND(taskbar as _))
}

/// Cursor position (physical screen px).
pub fn cursor() -> Option<(i32, i32)> {
    let mut p = POINT::default();
    unsafe { GetCursorPos(&mut p) }.ok().map(|_| (p.x, p.y))
}

/// Whether the primary mouse button is held down right now.
pub fn primary_button_down() -> bool {
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON, VK_RBUTTON};
    let swapped = unsafe { windows::Win32::UI::WindowsAndMessaging::GetSystemMetrics(SM_SWAPBUTTON) } != 0;
    let key = if swapped { VK_RBUTTON } else { VK_LBUTTON };
    (unsafe { GetAsyncKeyState(key.0 as i32) } as u16 & 0x8000) != 0
}

/// Windows other apps have docked into `taskbar` (AI Usage, system monitors…), as
/// `(left, right)` in its client x: visible direct children that belong neither to Explorer nor
/// to this process. Cheap (no UI Automation), so it is read on every placement.
pub fn others(taskbar: isize) -> Vec<(i32, i32)> {
    use windows::Win32::UI::WindowsAndMessaging::{EnumChildWindows, GetWindowThreadProcessId, IsWindowVisible};
    struct Scan {
        tb: HWND,
        explorer: u32,
        me: u32,
        out: Vec<(i32, i32)>,
    }
    unsafe extern "system" fn each(h: HWND, l: LPARAM) -> BOOL {
        let s = &mut *(l.0 as *mut Scan);
        let mut pid = 0u32;
        GetWindowThreadProcessId(h, Some(&mut pid));
        if GetParent(h).ok() == Some(s.tb) && pid != s.explorer && pid != s.me && IsWindowVisible(h).as_bool() {
            if let Some(r) = rect_of(h) {
                let mut p = POINT { x: r.left, y: r.top };
                if r.width() > 0 && ScreenToClient(s.tb, &mut p).as_bool() {
                    s.out.push((p.x, p.x + r.width()));
                }
            }
        }
        true.into()
    }
    let tb = HWND(taskbar as _);
    let mut explorer = 0u32;
    unsafe { windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId(tb, Some(&mut explorer)) };
    let mut scan = Scan { tb, explorer, me: std::process::id(), out: Vec::new() };
    unsafe {
        let _ = EnumChildWindows(Some(tb), Some(each), LPARAM(&mut scan as *mut Scan as isize));
    }
    scan.out.sort_unstable();
    scan.out
}

/// Where the taskbar's app icons, widgets button and notification area are, in its client x.
/// Read through UI Automation (Windows 11 draws the buttons in XAML, with no windows of their
/// own); without it only the notification area is known. Call on a thread with COM set up.
pub fn landmarks(taskbar: isize) -> Option<DockMarks> {
    let sp = space(taskbar)?;
    let mut marks = DockMarks { icons: None, widgets: None, tray: sp.right, others: others(taskbar) };
    let tb = HWND(taskbar as _);
    let mut origin = POINT::default();
    if !unsafe { ClientToScreen(tb, &mut origin) }.as_bool() {
        return Some(marks);
    }
    let mut read = || -> windows::core::Result<()> {
        let uia: IUIAutomation = unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER) }?;
        let root = unsafe { uia.ElementFromHandle(tb) }?;
        let frame_id = VARIANT::from("TaskbarFrame");
        let cond = unsafe { uia.CreatePropertyCondition(UIA_AutomationIdPropertyId, &frame_id) }?;
        let frame = unsafe { root.FindFirst(TreeScope_Descendants, &cond) }?;
        let all = unsafe { uia.CreateTrueCondition() }?;
        let kids = unsafe { frame.FindAll(TreeScope_Children, &all) }?;
        let mut icons: Option<(i32, i32)> = None;
        for i in 0..unsafe { kids.Length() }? {
            let e = unsafe { kids.GetElement(i) }?;
            let r = unsafe { e.CurrentBoundingRectangle() }?;
            if r.right <= r.left {
                continue;
            }
            let (l, rt) = (r.left - origin.x, r.right - origin.x);
            if unsafe { e.CurrentAutomationId() }.map(|s| s == "WidgetsButton").unwrap_or(false) {
                marks.widgets = Some((l, rt));
            } else {
                icons = Some(icons.map_or((l, rt), |(a, b)| (a.min(l), b.max(rt))));
            }
        }
        marks.icons = icons;
        Ok(())
    };
    if let Err(e) = read() {
        log::debug!("taskbar landmarks: {e}");
    }
    Some(marks)
}
