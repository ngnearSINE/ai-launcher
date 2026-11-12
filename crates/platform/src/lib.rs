//! Thin OS layer: running processes, window control, taskbar identity, shortcuts, icons.
//! Callers pass plain data (pids, paths, strings); nothing here knows about instances.

pub mod badge;
pub mod command;
pub mod dock_slot;
pub mod process;
pub mod snap;

pub use command::host_command;

#[cfg(windows)]
pub mod win;

#[cfg(target_os = "linux")]
pub mod linux;

/// An installed browser web app's shortcut: `(name, launcher, arguments, icon)`.
pub type WebAppShortcut = (String, std::path::PathBuf, String, Option<std::path::PathBuf>);

/// Shortcuts of installed Chromium web apps (anything started with `--app-id=`).
/// Windows: Start-menu `.lnk` pointing at `*_proxy.exe`; Linux: `.desktop` entries.
pub fn installed_web_apps() -> Vec<WebAppShortcut> {
    #[cfg(windows)]
    {
        win::shortcut::start_menu_links()
            .into_iter()
            .filter_map(|p| {
                let info = win::shortcut::read(&p)?;
                let exe = info.target.file_name()?.to_string_lossy().to_lowercase();
                if !exe.ends_with("_proxy.exe") || !info.args.contains("--app-id=") {
                    return None;
                }
                let name = p.file_stem()?.to_string_lossy().to_string();
                Some((name, info.target, info.args, info.icon.map(|i| i.0)))
            })
            .collect()
    }
    #[cfg(target_os = "linux")]
    {
        linux::web_app_entries()
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        Vec::new()
    }
}

/// A PNG icon for an installed program: Linux looks it up through the `.desktop` entry that
/// starts `exe` and the icon theme. Windows reads icons from the executable instead.
pub fn desktop_icon(exe: &std::path::Path) -> Option<std::path::PathBuf> {
    #[cfg(target_os = "linux")]
    {
        linux::icon_for_exe(exe)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = exe;
        None
    }
}

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
