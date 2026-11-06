use std::path::{Path, PathBuf};

use super::{alias_exists, detected, file_name_lower};
use crate::adapter::*;
use crate::env::{Env, Os};
use crate::model::{Installation, Instance};

/// Claude Desktop (Electron). Copies are isolated with `--user-data-dir`; the Windows Store
/// build is started through its `claude-desktop.exe` execution alias, which forwards arguments.
/// The `CLAUDE_USER_DATA_DIR` variable is ignored by signed builds, so it is never used.
pub struct ClaudeAdapter;

const DATA_DIR: &str = "Claude";

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
                    out.push(detected("claude:store", "claude", "Claude", alias));
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
        env.config.join(format!("{DATA_DIR}-{slug}"))
    }

    fn default_profile_dir(&self, _install: &Installation, env: &Env) -> Option<PathBuf> {
        Some(env.config.join(DATA_DIR))
    }

    /// The app writes logs to `%LOCALAPPDATA%\<profile folder name>` on Windows.
    fn companion_dirs(&self, _install: &Installation, profile: &Path, env: &Env) -> Vec<PathBuf> {
        match (env.os, profile.file_name()) {
            (Os::Windows, Some(name)) => vec![env.local.join(name)],
            _ => Vec::new(),
        }
    }

    /// `Claude-<name>` folders next to the default one that hold a real profile. The app's own
    /// variants (`Claude-3p`, `Claude Nest-3p`) are third-party-mode folders, not accounts.
    fn existing_profiles(&self, _install: &Installation, env: &Env) -> Vec<(String, PathBuf)> {
        let Ok(rd) = std::fs::read_dir(&env.config) else { return Vec::new() };
        let mut out: Vec<(String, PathBuf)> = rd
            .flatten()
            .filter(|e| e.path().is_dir())
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                let rest = name.strip_prefix("Claude-")?;
                if rest.is_empty() || rest.to_lowercase().ends_with("3p") || rest.eq_ignore_ascii_case("data") {
                    return None;
                }
                e.path().join("config.json").is_file().then(|| (rest.to_string(), e.path()))
            })
            .collect();
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
