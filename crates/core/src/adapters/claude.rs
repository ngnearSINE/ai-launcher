use std::path::{Path, PathBuf};

use super::{alias_exists, detected, file_name_lower, opt_str};
use crate::adapter::*;
use crate::env::{normalize_path, Env, Os};
use crate::model::{Installation, Instance};

/// Claude Desktop (Electron). Copies are isolated with `--user-data-dir`; the Windows Store
/// build is started through its `claude-desktop.exe` execution alias, which forwards arguments.
/// The `CLAUDE_USER_DATA_DIR` variable is ignored by signed builds, so it is never used.
///
/// The Store build virtualizes AppData writes: apart from a few folders its manifest excludes,
/// a profile under `%APPDATA%` is really written to
/// `%LOCALAPPDATA%\Packages\<family>\LocalCache\Roaming\…`. New profiles therefore go outside
/// AppData, and existing ones are looked up in both places.
pub struct ClaudeAdapter;

const DATA_DIR: &str = "Claude";

/// `Claude_xxxxxxxxxxxxx` — the package family behind the execution alias.
fn store_family(env: &Env) -> Option<String> {
    let apps = env.local.join("Microsoft").join("WindowsApps");
    std::fs::read_dir(apps).ok()?.flatten().find_map(|e| {
        let name = e.file_name().to_string_lossy().to_string();
        (name.starts_with("Claude_") && alias_exists(&e.path().join("claude-desktop.exe"))).then_some(name)
    })
}

fn family(install: &Installation) -> Option<&str> {
    opt_str(install, "packageFamily")
}

impl ClaudeAdapter {
    /// `Claude-<name>` profile folders directly under `root`.
    fn scan(root: &Path) -> Vec<(String, PathBuf)> {
        let Ok(rd) = std::fs::read_dir(root) else { return Vec::new() };
        rd.flatten()
            .filter(|e| e.path().is_dir())
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                let rest = name.strip_prefix("Claude-")?;
                // The app's own variants (`Claude-3p`, `Claude-Data`) are not accounts.
                if rest.is_empty() || rest.to_lowercase().ends_with("3p") || rest.eq_ignore_ascii_case("data") {
                    return None;
                }
                e.path().join("config.json").is_file().then(|| (rest.to_string(), PathBuf::from(&name)))
            })
            .collect()
    }
}

impl AppAdapter for ClaudeAdapter {
    fn kind(&self) -> &'static str {
        "claude"
    }

    fn detect(&self, env: &Env) -> Vec<Installation> {
        let mut out = Vec::new();
        match env.os {
            Os::Windows => {
                let alias = env.local.join("Microsoft").join("WindowsApps").join("claude-desktop.exe");
                if alias_exists(&alias) {
                    let mut i = detected("claude:store", "claude", "Claude", alias);
                    if let Some(f) = store_family(env) {
                        i.options.insert("packageFamily".into(), f.into());
                    }
                    out.push(i);
                }
                let squirrel = env.local.join("AnthropicClaude").join("claude.exe");
                if squirrel.is_file() {
                    out.push(detected("claude:classic", "claude", "Claude", squirrel));
                }
            }
            _ => {
                if let Some(p) = env.which("claude-desktop") {
                    out.push(detected("claude:linux", "claude", "Claude", p));
                }
            }
        }
        out
    }

    fn recognize(&self, exe: &Path, _env: &Env) -> Option<Installation> {
        let name = file_name_lower(exe);
        (name == "claude.exe" || name == "claude-desktop.exe" || name == "claude-desktop").then(|| {
            let mut i = detected(&format!("claude:{}", crate::model::new_id()), "claude", "Claude", exe.to_path_buf());
            i.source = crate::model::InstallSource::Manual;
            i
        })
    }

    fn new_profile_dir(&self, _install: &Installation, slug: &str, env: &Env) -> PathBuf {
        env.launcher_data.join("profiles").join(format!("{DATA_DIR}-{slug}"))
    }

    fn default_profile_dir(&self, _install: &Installation, env: &Env) -> Option<PathBuf> {
        Some(env.config.join(DATA_DIR))
    }

    fn profile_marker(&self) -> &'static str {
        "config.json"
    }

    fn profile_locations(&self, install: &Installation, dir: &Path, env: &Env) -> Vec<PathBuf> {
        let mut v = vec![dir.to_path_buf()];
        if let Some(virt) = family(install).and_then(|f| env.msix_virtual(dir, f)) {
            v.push(virt);
        }
        v
    }

    /// The app writes logs to `%LOCALAPPDATA%\<profile folder name>` (virtualized for the Store build).
    fn companion_dirs(&self, install: &Installation, profile: &Path, env: &Env) -> Vec<PathBuf> {
        let Some(name) = profile.file_name() else { return Vec::new() };
        if env.os != Os::Windows {
            return Vec::new();
        }
        let real = env.local.join(name);
        let mut v = vec![real.clone()];
        if let Some(virt) = family(install).and_then(|f| env.msix_virtual(&real, f)) {
            v.push(virt);
        }
        v
    }

    /// Existing `Claude-<name>` profiles next to the default one — in AppData itself, the Store
    /// package's private copy of it, and the launcher's own profile folder. Virtualized ones are
    /// reported under their AppData path, which is what the app must be given to find them.
    fn existing_profiles(&self, install: &Installation, env: &Env) -> Vec<(String, PathBuf)> {
        let mut out: Vec<(String, PathBuf)> = Vec::new();
        let mut roots: Vec<(PathBuf, PathBuf)> = vec![(env.config.clone(), env.config.clone())];
        if let Some(f) = family(install) {
            roots.push((env.local.join("Packages").join(f).join("LocalCache").join("Roaming"), env.config.clone()));
        }
        roots.push((env.launcher_data.join("profiles"), env.launcher_data.join("profiles")));
        for (scan_root, logical_root) in roots {
            for (alias, name) in Self::scan(&scan_root) {
                let logical = logical_root.join(&name);
                let key = normalize_path(&logical, env.os);
                if !out.iter().any(|(_, p)| normalize_path(p, env.os) == key) {
                    out.push((alias, logical));
                }
            }
        }
        out.sort();
        out
    }

    fn launch_plan(&self, install: &Installation, instance: &Instance, _ctx: &Ctx) -> Result<LaunchPlan, String> {
        let mut args = user_data_dir_args(instance);
        args.extend(instance.launch.args.iter().cloned());
        Ok(LaunchPlan { program: install.exe.clone(), args, env: instance.launch.env.clone(), cwd: None })
    }

    fn matcher(&self, _install: &Installation, instance: &Instance, ctx: &Ctx) -> ProcessMatcher {
        let names: &[&str] = match ctx.env.os {
            Os::Windows => &["claude.exe"],
            _ => &["claude-desktop", "claude"],
        };
        data_dir_matcher(names, instance, ctx.env)
    }

    fn usage(&self, _install: &Installation) -> Option<UsageKind> {
        Some(UsageKind::ClaudeDesktop)
    }

    fn brand(&self, _install: &Installation) -> Brand {
        Brand { color: "#D97757".into(), letter: "C".into(), logo: "claude".into() }
    }
}
