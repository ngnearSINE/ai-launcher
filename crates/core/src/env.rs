use std::path::{Path, PathBuf};

/// The handful of OS folders adapters need. Kept as plain data so detection and
/// launch planning can be unit-tested against a fake directory tree.
#[derive(Debug, Clone)]
pub struct Env {
    pub os: Os,
    pub home: PathBuf,
    /// Windows `%APPDATA%`, Linux `$XDG_CONFIG_HOME` or `~/.config`.
    pub config: PathBuf,
    /// Windows `%LOCALAPPDATA%`, Linux `$XDG_DATA_HOME` or `~/.local/share`.
    pub local: PathBuf,
    /// Windows `C:\Program Files`; unused on Linux.
    pub program_files: Vec<PathBuf>,
    /// Folders on `PATH`.
    pub path: Vec<PathBuf>,
    /// Where the launcher keeps profiles it creates for apps without their own convention.
    pub launcher_data: PathBuf,
    /// Installed browser web apps (PWAs) found by the platform layer (Start menu / .desktop).
    pub pwas: Vec<PwaShortcut>,
}

/// One installed Chromium web app, as its shortcut starts it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PwaShortcut {
    pub name: String,
    /// `chrome_proxy.exe` (Windows) or the browser binary (Linux).
    pub launcher: PathBuf,
    pub profile_directory: String,
    pub app_id: String,
    pub icon: Option<PathBuf>,
}

impl PwaShortcut {
    /// Parses `--profile-directory=… --app-id=…` from a shortcut's arguments.
    pub fn from_args(name: &str, launcher: PathBuf, args: &str, icon: Option<PathBuf>) -> Option<Self> {
        let mut profile = String::from("Default");
        let mut app_id = None;
        for tok in split_command_line(args) {
            if let Some(v) = tok.strip_prefix("--profile-directory=") {
                profile = v.to_string();
            } else if let Some(v) = tok.strip_prefix("--app-id=") {
                app_id = Some(v.to_string());
            }
        }
        Some(Self { name: name.to_string(), launcher, profile_directory: profile, app_id: app_id?, icon })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    Windows,
    Linux,
    Mac,
}

impl Env {
    pub fn detect() -> Self {
        let os = if cfg!(windows) {
            Os::Windows
        } else if cfg!(target_os = "macos") {
            Os::Mac
        } else {
            Os::Linux
        };
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let config = dirs::config_dir().unwrap_or_else(|| home.join(".config"));
        let local = dirs::data_local_dir().unwrap_or_else(|| home.join(".local/share"));
        let mut program_files = Vec::new();
        for key in ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"] {
            if let Some(v) = std::env::var_os(key) {
                let p = PathBuf::from(v);
                if !program_files.contains(&p) {
                    program_files.push(p);
                }
            }
        }
        let path = std::env::var_os("PATH")
            .map(|p| std::env::split_paths(&p).collect())
            .unwrap_or_default();
        // Profiles the launcher creates live outside AppData:
        // - Store (MSIX) apps such as Claude have their AppData writes redirected into the
        //   package's private LocalCache, where the launcher (and Explorer) would not find them;
        // - the installer's program folder (%LOCALAPPDATA%\AI Launcher) must not hold sign-ins
        //   that would go away with an uninstall.
        let launcher_data = home.join(".ai-launcher");
        Self { os, home, config, local, program_files, path, launcher_data, pwas: Vec::new() }
    }

    pub fn exe(&self, name: &str) -> String {
        if self.os == Os::Windows && !name.to_ascii_lowercase().ends_with(".exe") {
            format!("{name}.exe")
        } else {
            name.to_string()
        }
    }

    /// Where a Store (MSIX) package's writes to `path` actually land when the package has
    /// file-system write virtualization: `%LOCALAPPDATA%\Packages\<family>\LocalCache\Roaming|Local\…`.
    /// `None` for paths outside AppData.
    pub fn msix_virtual(&self, path: &Path, family: &str) -> Option<PathBuf> {
        if self.os != Os::Windows {
            return None;
        }
        let cache = self.local.join("Packages").join(family).join("LocalCache");
        if let Some(rest) = strip_prefix_ci(path, &self.config) {
            return Some(cache.join("Roaming").join(rest));
        }
        let rest = strip_prefix_ci(path, &self.local)?;
        // Already inside a package's own folder: nothing to redirect.
        if rest.components().next().map(|c| c.as_os_str().eq_ignore_ascii_case("Packages")).unwrap_or(false) {
            return None;
        }
        Some(cache.join("Local").join(rest))
    }

    /// First existing file among candidates.
    pub fn first_file<I: IntoIterator<Item = PathBuf>>(candidates: I) -> Option<PathBuf> {
        candidates.into_iter().find(|p| p.is_file())
    }

    /// Looks a program up on PATH (and common Linux user bins).
    pub fn which(&self, name: &str) -> Option<PathBuf> {
        let file = self.exe(name);
        let mut dirs: Vec<PathBuf> = self.path.clone();
        if self.os != Os::Windows {
            dirs.extend(
                ["/usr/bin", "/usr/local/bin", "/snap/bin", "/opt/google/chrome"]
                    .iter()
                    .map(PathBuf::from),
            );
            dirs.push(self.home.join(".local/bin"));
        }
        Self::first_file(dirs.into_iter().map(|d| d.join(&file)))
    }
}

/// Lower-cased, separator-normalised, trailing-slash-free form for comparing paths that
/// arrive from command lines (quotes, mixed slashes, different case on Windows).
pub fn normalize_path(p: &Path, os: Os) -> String {
    let mut s = p.to_string_lossy().trim().trim_matches('"').to_string();
    if os == Os::Windows {
        s = s.replace('/', "\\").to_lowercase();
        if let Some(rest) = s.strip_prefix("\\\\?\\") {
            s = rest.to_string();
        }
    }
    while s.len() > 3 && (s.ends_with('\\') || s.ends_with('/')) {
        s.pop();
    }
    s
}

/// Splits a command line on spaces outside double quotes and drops the quotes
/// (`--a="b c" d` → `--a=b c`, `d`).
pub fn split_command_line(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    for c in s.chars() {
        match c {
            '"' => quoted = !quoted,
            ' ' | '\t' if !quoted => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            _ => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// `path` relative to `base`, comparing components case-insensitively (Windows paths).
pub fn strip_prefix_ci(path: &Path, base: &Path) -> Option<PathBuf> {
    let mut p = path.components();
    for b in base.components() {
        let c = p.next()?;
        if !c.as_os_str().to_string_lossy().eq_ignore_ascii_case(&b.as_os_str().to_string_lossy()) {
            return None;
        }
    }
    Some(p.as_path().to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_pwa_shortcut_args() {
        let p = PwaShortcut::from_args("Gemini", PathBuf::from("chrome_proxy.exe"), r#" --profile-directory="Profile 1" --app-id=kidhffoijkgamgmpojncofndlmpihnoc"#, None).unwrap();
        assert_eq!(p.profile_directory, "Profile 1");
        assert_eq!(p.app_id, "kidhffoijkgamgmpojncofndlmpihnoc");
        let q = PwaShortcut::from_args("X", PathBuf::new(), "--profile-directory=Default --app-id=abc", None).unwrap();
        assert_eq!((q.profile_directory.as_str(), q.app_id.as_str()), ("Default", "abc"));
        assert!(PwaShortcut::from_args("Y", PathBuf::new(), "--profile-directory=Default", None).is_none());
    }

    #[test]
    #[cfg(windows)]
    fn maps_msix_virtual_paths() {
        let e = Env {
            os: Os::Windows,
            home: PathBuf::from(r"C:\Users\A"),
            config: PathBuf::from(r"C:\Users\A\AppData\Roaming"),
            local: PathBuf::from(r"C:\Users\A\AppData\Local"),
            program_files: vec![],
            path: vec![],
            launcher_data: PathBuf::from(r"C:\Users\A\.ai-launcher"),
            pwas: vec![],
        };
        assert_eq!(
            e.msix_virtual(Path::new(r"c:\users\a\appdata\roaming\Claude-Work"), "Claude_x"),
            Some(PathBuf::from(r"C:\Users\A\AppData\Local\Packages\Claude_x\LocalCache\Roaming\Claude-Work"))
        );
        assert_eq!(
            e.msix_virtual(Path::new(r"C:\Users\A\AppData\Local\Claude-Work"), "Claude_x"),
            Some(PathBuf::from(r"C:\Users\A\AppData\Local\Packages\Claude_x\LocalCache\Local\Claude-Work"))
        );
        assert_eq!(e.msix_virtual(Path::new(r"C:\Users\A\.ai-launcher\profiles\Claude-Work"), "Claude_x"), None);
        assert_eq!(e.msix_virtual(Path::new(r"C:\Users\A\AppData\Local\Packages\X\y"), "Claude_x"), None);
    }

    #[test]
    fn normalizes_windows_paths() {
        let a = normalize_path(Path::new("C:/Users/A/AppData/Roaming/Claude-Work/"), Os::Windows);
        let b = normalize_path(Path::new("\"c:\\users\\a\\appdata\\roaming\\claude-work\""), Os::Windows);
        assert_eq!(a, b);
    }
}
