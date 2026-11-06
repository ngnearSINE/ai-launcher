//! Linux desktop integration (freedesktop): per-instance `.desktop` entries whose
//! `StartupWMClass` matches the `--class` the launcher passes, so docks keep instances apart
//! and can pin each one.

use std::path::{Path, PathBuf};

use crate::host_command;

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

/// `$XDG_DATA_HOME` first, then `$XDG_DATA_DIRS` (freedesktop defaults when unset) and the
/// Flatpak export folders.
fn data_dirs() -> Vec<PathBuf> {
    let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
    let mut dirs = vec![applications_dir().parent().map(Path::to_path_buf).unwrap_or_else(|| home.join(".local/share"))];
    let system = std::env::var("XDG_DATA_DIRS").ok().filter(|s| !s.trim().is_empty()).unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    dirs.extend(system.split(':').filter(|s| !s.is_empty()).map(PathBuf::from));
    dirs.push(home.join(".local/share/flatpak/exports/share"));
    dirs.push(PathBuf::from("/var/lib/flatpak/exports/share"));
    let mut seen = std::collections::HashSet::new();
    dirs.retain(|d| seen.insert(d.clone()));
    dirs
}

/// One application from a `.desktop` file (`[Desktop Entry]` group only).
#[derive(Debug, Clone)]
pub struct DesktopApp {
    pub file: PathBuf,
    pub name: String,
    pub program: PathBuf,
    /// Arguments after the program, field codes (`%U`, `%f`, …) removed, as written.
    pub args: String,
    pub icon: Option<String>,
}

/// Splits an `Exec=` value into words, honouring double quotes and dropping field codes.
fn exec_words(exec: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut chars = exec.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => quoted = !quoted,
            '\\' if quoted => {
                if let Some(n) = chars.next() {
                    cur.push(n);
                }
            }
            c if c.is_whitespace() && !quoted => {
                if !cur.is_empty() {
                    words.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        words.push(cur);
    }
    words.retain(|w| !(w.len() == 2 && w.starts_with('%')));
    words
}

fn parse_desktop(file: &Path) -> Option<DesktopApp> {
    let text = std::fs::read_to_string(file).ok()?;
    let mut in_entry = false;
    let (mut name, mut exec, mut icon) = (None, None, None);
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_entry {
            continue;
        }
        if let Some(v) = line.strip_prefix("Name=") {
            name.get_or_insert_with(|| v.trim().to_string());
        } else if let Some(v) = line.strip_prefix("Exec=") {
            exec = Some(v.trim().to_string());
        } else if let Some(v) = line.strip_prefix("Icon=") {
            icon = Some(v.trim().to_string()).filter(|s| !s.is_empty());
        }
    }
    let words = exec_words(&exec?);
    let (program, rest) = words.split_first()?;
    // `env VAR=x prog …` is common in packaged entries.
    let (program, rest) = if program == "env" || program.ends_with("/env") {
        let i = rest.iter().position(|w| !w.contains('='))?;
        (&rest[i], &rest[i + 1..])
    } else {
        (program, rest)
    };
    Some(DesktopApp { file: file.to_path_buf(), name: name?, program: PathBuf::from(program), args: rest.join(" "), icon })
}

/// Every application entry the session can see, user entries first.
pub fn desktop_apps() -> Vec<DesktopApp> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for dir in data_dirs() {
        let Ok(rd) = std::fs::read_dir(dir.join("applications")) else { continue };
        for e in rd.flatten() {
            let path = e.path();
            if path.extension().map(|x| x == "desktop").unwrap_or(false) && seen.insert(e.file_name()) {
                if let Some(app) = parse_desktop(&path) {
                    out.push(app);
                }
            }
        }
    }
    out
}

/// Chromium web-app entries (`Exec=… --app-id=…`).
pub fn web_app_entries() -> Vec<crate::WebAppShortcut> {
    desktop_apps()
        .into_iter()
        .filter(|a| a.args.contains("--app-id="))
        .map(|a| {
            let icon = a.icon.as_deref().and_then(resolve_icon);
            (a.name, a.program, a.args, icon)
        })
        .collect()
}

/// A program name from `PATH`, or the path itself.
fn resolve_program(p: &Path) -> Option<PathBuf> {
    if p.components().count() > 1 {
        return std::fs::canonicalize(p).ok();
    }
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).map(|d| d.join(p)).find(|c| c.is_file()).and_then(|c| std::fs::canonicalize(c).ok())
}

/// The icon of the `.desktop` entry that starts `exe` (same file after resolving links, or the
/// same program name when the entry uses a wrapper).
pub fn icon_for_exe(exe: &Path) -> Option<PathBuf> {
    let target = std::fs::canonicalize(exe).unwrap_or_else(|_| exe.to_path_buf());
    let stem = |p: &Path| p.file_stem().map(|s| s.to_string_lossy().to_lowercase());
    let apps: Vec<DesktopApp> = desktop_apps().into_iter().filter(|a| a.icon.is_some() && !a.args.contains("--app-id=")).collect();
    let exact = apps.iter().find(|a| resolve_program(&a.program).as_deref() == Some(target.as_path()));
    let by_name = || apps.iter().find(|a| stem(&a.program).is_some() && (stem(&a.program) == stem(exe) || stem(&a.program) == stem(&target)));
    exact.or_else(by_name).and_then(|a| resolve_icon(a.icon.as_deref()?))
}

/// Resolves an `Icon=` value to a PNG: an absolute path, or a name looked up in the hicolor
/// theme (largest bitmap first) and `pixmaps`.
pub fn resolve_icon(icon: &str) -> Option<PathBuf> {
    let p = Path::new(icon);
    if p.is_absolute() {
        return (p.is_file() && p.extension().map(|e| e.eq_ignore_ascii_case("png")).unwrap_or(false)).then(|| p.to_path_buf());
    }
    const SIZES: &[&str] = &["256x256", "512x512", "192x192", "128x128", "96x96", "72x72", "64x64", "48x48"];
    let file = format!("{icon}.png");
    let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
    let mut roots: Vec<PathBuf> = vec![home.join(".icons")];
    roots.extend(data_dirs().into_iter().map(|d| d.join("icons")));
    for size in SIZES {
        for root in &roots {
            let c = root.join("hicolor").join(size).join("apps").join(&file);
            if c.is_file() {
                return Some(c);
            }
        }
    }
    data_dirs().into_iter().map(|d| d.join("pixmaps").join(&file)).find(|c| c.is_file())
}

/// Uses `wmctrl` when installed (X11 / XWayland); on pure Wayland focusing another app's window
/// is not permitted and this returns false.
pub fn focus_any(pids: &[u32]) -> bool {
    let Ok(out) = host_command("wmctrl").arg("-lp").output() else { return false };
    let list = String::from_utf8_lossy(&out.stdout);
    for line in list.lines() {
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() > 2 && cols[2].parse::<u32>().map(|p| pids.contains(&p)).unwrap_or(false) {
            return host_command("wmctrl").args(["-ia", cols[0]]).status().map(|s| s.success()).unwrap_or(false);
        }
    }
    false
}

pub fn terminate(pids: &[u32]) -> bool {
    let mut ok = false;
    for p in pids {
        ok |= host_command("kill").args(["-TERM", &p.to_string()]).status().map(|s| s.success()).unwrap_or(false);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exec_words_handles_quotes_and_field_codes() {
        assert_eq!(exec_words(r#""/opt/Grok Desktop/grok-desktop" %U"#), vec!["/opt/Grok Desktop/grok-desktop"]);
        assert_eq!(exec_words("grok-bot -- %U"), vec!["grok-bot", "--"]);
        assert_eq!(
            exec_words("/opt/google/chrome/google-chrome --profile-directory=Default --app-id=abc"),
            vec!["/opt/google/chrome/google-chrome", "--profile-directory=Default", "--app-id=abc"]
        );
    }

    #[test]
    fn desktop_entry_reads_only_the_main_group() {
        let dir = std::env::temp_dir().join(format!("ail-desktop-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("x.desktop");
        std::fs::write(&file, "[Desktop Entry]\nName=Grok Desktop\nExec=env A=1 \"/opt/Grok Desktop/grok-desktop\" %U\nIcon=grok-desktop\n[Desktop Action New]\nName=New\nExec=other\n").unwrap();
        let app = parse_desktop(&file).unwrap();
        assert_eq!(app.name, "Grok Desktop");
        assert_eq!(app.program, PathBuf::from("/opt/Grok Desktop/grok-desktop"));
        assert_eq!(app.icon.as_deref(), Some("grok-desktop"));
        let _ = std::fs::remove_dir_all(dir);
    }
}
