use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Serialize;

use crate::env::{normalize_path, Env, Os};
use crate::model::{Installation, Instance};

/// What the app crate needs to start a process.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchPlan {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub cwd: Option<PathBuf>,
}

/// How copies of an app are kept apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Isolation {
    /// `--user-data-dir=<folder>` (Electron and Chromium).
    UserDataDir,
    /// The app has no known switch; every instance runs the same data.
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum UsageKind {
    /// Claude Desktop: the OAuth token cache inside that instance's own profile folder.
    ClaudeDesktop,
    /// Cursor: the sign-in in `User/globalStorage/state.vscdb` of that instance's folder.
    Cursor,
    /// Grok Bot: the Cursor account in `sand-secrets.json` of that instance's folder.
    GrokBot,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Brand {
    pub color: String,
    pub letter: String,
    /// Key of the logo the UI draws (`claude`, `chrome`, `gemini`, `grok`, `orca`, `generic`).
    pub logo: String,
}

/// Recognises the main process of one instance among running processes.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessMatcher {
    /// Lower-case executable file names (`claude.exe`, `chrome`).
    pub exe_names: Vec<String>,
    /// Full executable path when the name alone is ambiguous (custom programs).
    pub exe_path: Option<String>,
    /// Normalised profile folder that must appear as `--user-data-dir`; `None` = the default
    /// profile, which matches only processes started *without* that switch.
    pub data_dir: Option<String>,
    /// Whether the switch is used at all. When false any process with a matching name counts.
    pub uses_data_dir: bool,
}

impl ProcessMatcher {
    /// `args` excludes argv[0]. Child processes of Chromium/Electron carry `--type=`; skip them.
    pub fn matches(&self, exe_name: &str, exe_path: Option<&Path>, args: &[String], os: Os) -> bool {
        let name = exe_name.to_lowercase();
        if !self.exe_names.iter().any(|n| *n == name) {
            return false;
        }
        if let (Some(want), Some(have)) = (&self.exe_path, exe_path) {
            if normalize_path(have, os) != *want {
                return false;
            }
        }
        if args.iter().any(|a| a.starts_with("--type=")) {
            return false;
        }
        if !self.uses_data_dir {
            return true;
        }
        let found = user_data_dir_arg(args).map(|d| normalize_path(Path::new(&d), os));
        found == self.data_dir
    }
}

/// Value of `--user-data-dir=…` or `--user-data-dir …` in an argument list.
pub fn user_data_dir_arg(args: &[String]) -> Option<String> {
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if let Some(v) = a.strip_prefix("--user-data-dir=") {
            return Some(v.trim_matches('"').to_string());
        }
        if a == "--user-data-dir" {
            return it.next().map(|v| v.trim_matches('"').to_string());
        }
    }
    None
}

/// Everything an adapter may look at while planning.
pub struct Ctx<'a> {
    pub env: &'a Env,
    pub installations: &'a [Installation],
}

impl<'a> Ctx<'a> {
    pub fn installation(&self, id: &str) -> Option<&'a Installation> {
        self.installations.iter().find(|i| i.id == id)
    }
}

/// One family of apps. Implementations only describe; they never spawn or read processes.
pub trait AppAdapter: Send + Sync {
    /// Stable identifier stored in `Installation::adapter`.
    fn kind(&self) -> &'static str;

    /// Installed copies found in the usual locations.
    fn detect(&self, env: &Env) -> Vec<Installation>;

    /// Turns an executable the user picked into an installation, if this adapter recognises it.
    fn recognize(&self, _exe: &Path, _env: &Env) -> Option<Installation> {
        None
    }

    fn isolation(&self, _install: &Installation) -> Isolation {
        Isolation::UserDataDir
    }

    /// Whether an instance may use the app's own default folder (false for web apps,
    /// which always get a launcher-owned browser folder).
    fn supports_default_profile(&self, _install: &Installation) -> bool {
        true
    }

    /// The folder a new instance called `slug` should use.
    fn new_profile_dir(&self, install: &Installation, slug: &str, env: &Env) -> PathBuf {
        env.launcher_data.join("profiles").join(format!("{}-{slug}", install_slug(install)))
    }

    /// Extra folders the app creates next to a custom profile (logs, caches), for clean removal.
    fn companion_dirs(&self, _install: &Installation, _profile: &Path, _env: &Env) -> Vec<PathBuf> {
        Vec::new()
    }

    /// Every physical place a profile folder's data can be in. Normally just `dir`; a Store
    /// package with write virtualization may keep it in its private LocalCache instead.
    fn profile_locations(&self, _install: &Installation, dir: &Path, _env: &Env) -> Vec<PathBuf> {
        vec![dir.to_path_buf()]
    }

    /// The location that actually holds the profile (the first with the app's marker file,
    /// else the first that exists, else `dir`). Used for reading usage, sizes and opening folders.
    fn resolve_profile_dir(&self, install: &Installation, dir: &Path, env: &Env) -> PathBuf {
        let locs = self.profile_locations(install, dir, env);
        locs.iter()
            .find(|p| p.join(self.profile_marker()).is_file())
            .or_else(|| locs.iter().find(|p| p.is_dir()))
            .cloned()
            .unwrap_or_else(|| dir.to_path_buf())
    }

    /// File whose presence means "this folder has been used by the app".
    fn profile_marker(&self) -> &'static str {
        "Local State"
    }

    /// Profile folders that already exist on disk (made by hand or by an earlier tool), as
    /// `(suggested alias, folder)`.
    fn existing_profiles(&self, _install: &Installation, _env: &Env) -> Vec<(String, PathBuf)> {
        Vec::new()
    }

    /// The folder the app uses when started normally, if known.
    fn default_profile_dir(&self, _install: &Installation, _env: &Env) -> Option<PathBuf> {
        None
    }

    fn launch_plan(&self, install: &Installation, instance: &Instance, ctx: &Ctx) -> Result<LaunchPlan, String>;

    fn matcher(&self, install: &Installation, instance: &Instance, ctx: &Ctx) -> ProcessMatcher;

    fn usage(&self, _install: &Installation) -> Option<UsageKind> {
        None
    }

    fn brand(&self, install: &Installation) -> Brand;
}

/// Folder-name stem for an installation (`claude`, `chrome`, `gemini`).
pub fn install_slug(install: &Installation) -> String {
    crate::text::slug(&install.name, &install.adapter).to_lowercase()
}

/// Shared helper for adapters whose isolation is `--user-data-dir`.
pub fn user_data_dir_args(instance: &Instance) -> Vec<String> {
    match instance.profile.dir() {
        Some(dir) => vec![format!("--user-data-dir={}", dir.display())],
        None => Vec::new(),
    }
}

pub fn data_dir_matcher(exe_names: &[&str], instance: &Instance, env: &Env) -> ProcessMatcher {
    ProcessMatcher {
        exe_names: exe_names.iter().map(|s| s.to_lowercase()).collect(),
        exe_path: None,
        data_dir: instance.profile.dir().map(|d| normalize_path(d, env.os)),
        uses_data_dir: true,
    }
}

/// All adapters, looked up by kind.
#[derive(Clone)]
pub struct Registry {
    adapters: Vec<Arc<dyn AppAdapter>>,
}

impl Registry {
    pub fn new(adapters: Vec<Arc<dyn AppAdapter>>) -> Self {
        Self { adapters }
    }

    pub fn standard() -> Self {
        use crate::adapters::*;
        Self::new(vec![
            Arc::new(claude::ClaudeAdapter),
            Arc::new(electron::ElectronAdapter),
            Arc::new(chromium::ChromiumAdapter),
            Arc::new(webapp::WebAppAdapter),
            Arc::new(custom::CustomAdapter),
        ])
    }

    pub fn get(&self, kind: &str) -> Option<&Arc<dyn AppAdapter>> {
        self.adapters.iter().find(|a| a.kind() == kind)
    }

    pub fn all(&self) -> &[Arc<dyn AppAdapter>] {
        &self.adapters
    }

    /// First adapter that recognises the executable; `custom` accepts anything.
    pub fn recognize(&self, exe: &Path, env: &Env) -> Option<Installation> {
        self.adapters.iter().find_map(|a| a.recognize(exe, env))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_user_data_dir() {
        let a = vec!["--foo".into(), "--user-data-dir=C:\\x\\y".into()];
        assert_eq!(user_data_dir_arg(&a).as_deref(), Some("C:\\x\\y"));
        let b = vec!["--user-data-dir".into(), "\"/home/a/p\"".into()];
        assert_eq!(user_data_dir_arg(&b).as_deref(), Some("/home/a/p"));
    }

    #[test]
    fn matcher_distinguishes_default_and_custom() {
        let m = ProcessMatcher {
            exe_names: vec!["claude.exe".into()],
            exe_path: None,
            data_dir: None,
            uses_data_dir: true,
        };
        assert!(m.matches("Claude.exe", None, &[], Os::Windows));
        assert!(!m.matches("Claude.exe", None, &["--user-data-dir=C:\\p".into()], Os::Windows));
        assert!(!m.matches("Claude.exe", None, &["--type=renderer".into()], Os::Windows));
        let c = ProcessMatcher { data_dir: Some("c:\\p".into()), ..m };
        assert!(c.matches("claude.exe", None, &["--user-data-dir=C:\\P\\".into()], Os::Windows));
    }
}
