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
        Self { os, home, config, local, program_files, path, launcher_data }
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
