//! Commands for programs outside the launcher.
//!
//! An AppImage starts the launcher with variables that point into its own mounted bundle
//! (`LD_LIBRARY_PATH`, `PYTHONHOME`, `GI_TYPELIB_PATH`, `GTK_*`, …). The launcher's own web view
//! needs them, but a program it starts (Claude, Cursor, `python3`) would load the bundle's
//! libraries instead of its own and fail. Every external program is started through
//! [`host_command`], which gives it the environment the user's session had.

use std::ffi::{OsStr, OsString};
use std::process::Command;

/// Variables an AppImage runtime sets that have no meaning outside the bundle.
const APPIMAGE_ONLY: &[&str] = &["APPDIR", "APPIMAGE", "ARGV0", "OWD", "GTK_THEME", "PYTHONDONTWRITEBYTECODE"];

/// `Command::new(program)` with the AppImage bundle removed from its environment.
pub fn host_command(program: impl AsRef<OsStr>) -> Command {
    let mut c = Command::new(program);
    for (key, value) in host_env_changes(std::env::vars_os()) {
        match value {
            Some(v) => c.env(key, v),
            None => c.env_remove(key),
        };
    }
    c
}

/// What to change in `vars` so a child does not see the AppImage bundle:
/// `(name, None)` removes a variable, `(name, Some(v))` replaces it.
pub fn host_env_changes(vars: impl Iterator<Item = (OsString, OsString)>) -> Vec<(OsString, Option<OsString>)> {
    let vars: Vec<(OsString, OsString)> = vars.collect();
    let Some(appdir) = vars.iter().find(|(k, _)| k == "APPDIR").map(|(_, v)| v.to_string_lossy().trim_end_matches('/').to_string()) else {
        return Vec::new();
    };
    if appdir.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for (key, value) in &vars {
        let name = key.to_string_lossy();
        if APPIMAGE_ONLY.contains(&name.as_ref()) {
            out.push((key.clone(), None));
            continue;
        }
        let text = value.to_string_lossy();
        if !text.contains(&appdir) {
            continue;
        }
        // Path lists keep the user's own entries; a single value inside the bundle goes.
        let kept: Vec<&str> = text.split(':').filter(|p| !p.is_empty() && !p.starts_with(&appdir)).collect();
        if kept.is_empty() {
            out.push((key.clone(), None));
        } else {
            out.push((key.clone(), Some(OsString::from(kept.join(":")))));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars(list: &[(&str, &str)]) -> impl Iterator<Item = (OsString, OsString)> {
        list.iter().map(|(k, v)| (OsString::from(k), OsString::from(v))).collect::<Vec<_>>().into_iter()
    }

    #[test]
    fn outside_an_appimage_nothing_changes() {
        assert!(host_env_changes(vars(&[("LD_LIBRARY_PATH", "/opt/x"), ("PATH", "/usr/bin")])).is_empty());
    }

    #[test]
    fn bundle_paths_are_dropped_and_user_paths_kept() {
        let changes = host_env_changes(vars(&[
            ("APPDIR", "/tmp/.mount_ail/"),
            ("LD_LIBRARY_PATH", "/tmp/.mount_ail/usr/lib/:/tmp/.mount_ail/usr/lib/x86_64-linux-gnu/"),
            ("PATH", "/tmp/.mount_ail/usr/bin:/usr/local/bin:/usr/bin"),
            ("PYTHONHOME", "/tmp/.mount_ail/usr/"),
            ("GTK_THEME", "Adwaita:dark"),
            ("HOME", "/home/u"),
        ]));
        let get = |k: &str| changes.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone());
        assert_eq!(get("LD_LIBRARY_PATH"), Some(None));
        assert_eq!(get("PYTHONHOME"), Some(None));
        assert_eq!(get("GTK_THEME"), Some(None));
        assert_eq!(get("APPDIR"), Some(None));
        assert_eq!(get("PATH"), Some(Some(OsString::from("/usr/local/bin:/usr/bin"))));
        assert_eq!(get("HOME"), None);
    }
}
