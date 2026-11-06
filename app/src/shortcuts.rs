//! Desktop / Start-menu shortcuts that start one instance through the launcher.

use std::path::PathBuf;

use launcher_core::Instance;

use crate::core::Core;

fn safe_name(s: &str) -> String {
    let cleaned: String = s.chars().map(|c| if "\\/:*?\"<>|".contains(c) || c.is_control() { '_' } else { c }).collect();
    let t = cleaned.trim().trim_matches('.').to_string();
    if t.is_empty() {
        "Instance".into()
    } else {
        t
    }
}

fn label(core: &Core, inst: &Instance) -> String {
    let store = core.store.lock();
    let app = store.data.installations.iter().find(|i| i.id == inst.installation_id).map(|i| i.name.clone()).unwrap_or_default();
    if app.is_empty() || inst.alias.contains(&app) {
        inst.alias.clone()
    } else {
        format!("{} ({})", app, inst.alias)
    }
}

#[cfg(windows)]
pub fn start_menu_path(core: &Core, inst: &Instance) -> PathBuf {
    core.env
        .config
        .join("Microsoft/Windows/Start Menu/Programs/AI Launcher")
        .join(format!("{}.lnk", safe_name(&label(core, inst))))
}

#[cfg(not(windows))]
pub fn start_menu_path(_core: &Core, inst: &Instance) -> PathBuf {
    launcher_platform::linux::applications_dir().join(format!("ai-launcher-{}.desktop", inst.id))
}

#[cfg(windows)]
pub fn create(core: &Core, id: &str, place: &str) -> Result<PathBuf, String> {
    use launcher_core::catalog::effective;
    use launcher_platform::win::shortcut::{self, Shortcut};

    let inst = core.instance(id).ok_or("instance-missing")?;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let name = label(core, &inst);
    let path = match place {
        "desktop" => dirs_desktop().ok_or("no-desktop")?.join(format!("{}.lnk", safe_name(&name))),
        _ => start_menu_path(core, &inst),
    };
    let eff = effective(&core.settings(), &inst);
    let app_exe = core.run_info(id).and_then(|r| r.exe);
    let icon = if eff.taskbar_badge {
        crate::taskbar::badge_icon(core, &inst.id, &inst.color, app_exe.as_deref())
    } else {
        app_exe.clone()
    };
    let app_id = crate::taskbar::app_id(&inst.id);
    let args = format!("launch {}", inst.id);
    shortcut::create(
        &path,
        &Shortcut {
            target: &exe,
            args: &args,
            description: &name,
            icon: icon.as_deref().map(|p| (p, 0)),
            app_id: eff.taskbar_separate.then_some(app_id.as_str()),
        },
    )?;
    Ok(path)
}

#[cfg(windows)]
fn dirs_desktop() -> Option<PathBuf> {
    use windows_known_folder::desktop;
    desktop()
}

#[cfg(windows)]
mod windows_known_folder {
    use std::path::PathBuf;
    /// The real Desktop folder (follows OneDrive redirection).
    pub fn desktop() -> Option<PathBuf> {
        let out = std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-Command", "[Console]::OutputEncoding=[Text.Encoding]::UTF8; [Environment]::GetFolderPath('Desktop')"])
            .output()
            .ok()?;
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        (!s.is_empty()).then(|| PathBuf::from(s))
    }
}

#[cfg(not(windows))]
pub fn create(core: &Core, id: &str, place: &str) -> Result<PathBuf, String> {
    use launcher_platform::{badge, linux};
    let inst = core.instance(id).ok_or("instance-missing")?;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let name = label(core, &inst);
    let rgb = badge::parse_hex(&inst.color);
    let icon = core.icons_dir().join(format!("{}.png", inst.id));
    let _ = badge::write_png(&badge::plain_tile(256, rgb), rgb, 256, &icon);
    let is_web = {
        let store = core.store.lock();
        store.data.installations.iter().any(|i| i.id == inst.installation_id && i.adapter == "webapp")
    };
    let class = format!("AiLauncher-{}", inst.id);
    let args = vec!["launch".to_string(), inst.id.clone()];
    let entry = linux::DesktopEntry {
        file_stem: &format!("ai-launcher-{}", inst.id),
        name: &name,
        comment: "AI Launcher",
        exec: &exe,
        args: &args,
        icon: Some(&icon),
        wm_class: is_web.then_some(class.as_str()),
    };
    let path = linux::write_entry(&entry)?;
    if place == "desktop" {
        let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
        let target = home.join("Desktop").join(format!("ai-launcher-{}.desktop", inst.id));
        std::fs::copy(&path, &target).map_err(|e| e.to_string())?;
        return Ok(target);
    }
    Ok(path)
}
