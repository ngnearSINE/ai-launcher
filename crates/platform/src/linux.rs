//! Linux desktop integration (freedesktop): per-instance `.desktop` entries whose
//! `StartupWMClass` matches the `--class` the launcher passes, so docks keep instances apart
//! and can pin each one.

use std::path::{Path, PathBuf};
use std::process::Command;

pub fn applications_dir() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/share"))
        .join("applications")
}

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\n', " ")
}

fn quote_arg(a: &str) -> String {
    if a.chars().all(|c| c.is_ascii_alphanumeric() || "-_./=:".contains(c)) {
        a.to_string()
    } else {
        format!("\"{}\"", a.replace('\\', "\\\\").replace('"', "\\\"").replace('`', "\\`").replace('$', "\\$"))
    }
}

pub struct DesktopEntry<'a> {
    pub file_stem: &'a str,
    pub name: &'a str,
    pub comment: &'a str,
    pub exec: &'a Path,
    pub args: &'a [String],
    pub icon: Option<&'a Path>,
    pub wm_class: Option<&'a str>,
}

pub fn write_entry(e: &DesktopEntry) -> Result<PathBuf, String> {
    let dir = applications_dir();
    std::fs::create_dir_all(&dir).map_err(|x| x.to_string())?;
    let mut exec = quote_arg(&e.exec.to_string_lossy());
    for a in e.args {
        exec.push(' ');
        exec.push_str(&quote_arg(a));
    }
    let mut body = format!(
        "[Desktop Entry]\nType=Application\nName={}\nComment={}\nExec={}\nTerminal=false\nCategories=Utility;\n",
        escape(e.name),
        escape(e.comment),
        exec
    );
    if let Some(icon) = e.icon {
        body.push_str(&format!("Icon={}\n", icon.display()));
    }
    if let Some(class) = e.wm_class {
        body.push_str(&format!("StartupWMClass={class}\n"));
    }
    let path = dir.join(format!("{}.desktop", e.file_stem));
    std::fs::write(&path, body).map_err(|x| x.to_string())?;
    Ok(path)
}

pub fn remove_entry(file_stem: &str) {
    let _ = std::fs::remove_file(applications_dir().join(format!("{file_stem}.desktop")));
}

/// Uses `wmctrl` when installed (X11 / XWayland); on pure Wayland focusing another app's window
/// is not permitted and this returns false.
pub fn focus_any(pids: &[u32]) -> bool {
    let Ok(out) = Command::new("wmctrl").arg("-lp").output() else { return false };
    let list = String::from_utf8_lossy(&out.stdout);
    for line in list.lines() {
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() > 2 && cols[2].parse::<u32>().map(|p| pids.contains(&p)).unwrap_or(false) {
            return Command::new("wmctrl").args(["-ia", cols[0]]).status().map(|s| s.success()).unwrap_or(false);
        }
    }
    false
}

pub fn terminate(pids: &[u32]) -> bool {
    let mut ok = false;
    for p in pids {
        ok |= Command::new("kill").args(["-TERM", &p.to_string()]).status().map(|s| s.success()).unwrap_or(false);
    }
    ok
}
