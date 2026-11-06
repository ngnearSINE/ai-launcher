//! Thin OS layer: running processes, window control, taskbar identity, shortcuts, icons.
//! Callers pass plain data (pids, paths, strings); nothing here knows about instances.

pub mod badge;
pub mod process;

#[cfg(windows)]
pub mod win;

#[cfg(target_os = "linux")]
pub mod linux;

/// Brings the first visible top-level window of any of `pids` to the front.
pub fn focus_pids(pids: &[u32]) -> bool {
    #[cfg(windows)]
    {
        win::windows::focus_any(pids)
    }
    #[cfg(target_os = "linux")]
    {
        linux::focus_any(pids)
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        let _ = pids;
        false
    }
}

/// Asks the windows of `pids` to close (like clicking ×); apps may keep running in their tray.
pub fn close_pids(pids: &[u32]) -> bool {
    #[cfg(windows)]
    {
        win::windows::close_all(pids)
    }
    #[cfg(target_os = "linux")]
    {
        linux::terminate(pids)
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        let _ = pids;
        false
    }
}
