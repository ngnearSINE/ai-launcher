use std::path::{Path, PathBuf};

use super::{detected, opt_str};
use crate::adapter::*;
use crate::env::{Env, Os};
use crate::model::{InstallSource, Installation, Instance};

/// Any Electron app. Electron honours `--user-data-dir`, so every copy gets its own folder.
/// Known apps are detected in their default install locations; any other Electron app can be
/// added by path (recognised by its `resources/app.asar`).
pub struct ElectronAdapter;

struct Known {
    id: &'static str,
    name: &'static str,
    data_dir: &'static str,
    logo: &'static str,
    color: &'static str,
    win: &'static [&'static str],
    linux: &'static [&'static str],
}

const KNOWN: &[Known] = &[
    Known {
        id: "orca",
        name: "Orca",
        data_dir: "orca",
        logo: "orca",
        color: "#3B82F6",
        win: &["Programs/orca/Orca.exe"],
        linux: &["/opt/Orca/orca", "/usr/bin/orca", "/usr/lib/orca/orca"],
    },
    Known {
        id: "grok-bot",
        name: "Grok Bot",
        data_dir: "Grok Bot",
        logo: "grok",
        color: "#A1A1AA",
        win: &["Programs/Grok Bot/Grok Bot.exe"],
        linux: &["/opt/Grok Bot/grok-bot", "/usr/bin/grok-bot"],
    },
    Known {
        id: "poe",
        name: "Poe",
        data_dir: "Poe",
        logo: "poe",
        color: "#5D5CDE",
        // Squirrel install: the stub forwards its arguments to the newest app-<version>\Poe.exe.
        win: &["Poe/Poe.exe"],
        linux: &[],
    },
    Known {
        id: "cursor",
        name: "Cursor",
        data_dir: "Cursor",
        logo: "cursor",
        color: "#8E8E93",
        win: &["Programs/cursor/Cursor.exe"],
        // The Linux build is usually an AppImage, which passes its arguments on.
        linux: &["/usr/bin/cursor", "/opt/Cursor/cursor", "/opt/cursor.appimage", "/opt/cursor/cursor.appimage"],
    },
    Known {
        id: "grok-desktop",
        name: "Grok Desktop",
        data_dir: "Grok-Desktop",
        logo: "grok",
        color: "#A1A1AA",
        win: &[],
        linux: &["/opt/Grok Desktop/grok-desktop", "/usr/bin/grok-desktop"],
    },
];

fn electron_install(id: String, name: &str, exe: PathBuf, data_dir: &str, logo: &str, color: &str) -> Installation {
    let mut i = detected(&id, "electron", name, exe);
    i.options.insert("dataDirName".into(), data_dir.into());
    i.options.insert("logo".into(), logo.into());
    i.options.insert("color".into(), color.into());
    i
}

/// An Electron app ships `resources/app.asar` (or an unpacked `resources/app`) next to its exe.
/// Squirrel installs (`%LOCALAPPDATA%\<App>\<App>.exe`) keep it under `app-<version>\resources`.
pub fn looks_like_electron(exe: &Path) -> bool {
    let has = |d: &Path| d.join("resources").join("app.asar").is_file() || d.join("resources").join("app").is_dir();
    let Some(dir) = exe.parent() else { return false };
    if has(dir) {
        return true;
    }
    std::fs::read_dir(dir)
        .map(|rd| rd.flatten().any(|e| e.file_name().to_string_lossy().starts_with("app-") && has(&e.path())))
        .unwrap_or(false)
}

/// An Electron executable (after following links) or an AppImage, which bundles one.
fn is_electron_program(p: &Path) -> bool {
    let appimage = p.extension().map(|e| e.eq_ignore_ascii_case("appimage")).unwrap_or(false);
    appimage || looks_like_electron(&std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf()))
}

impl AppAdapter for ElectronAdapter {
    fn kind(&self) -> &'static str {
        "electron"
    }

    fn detect(&self, env: &Env) -> Vec<Installation> {
        let mut out = Vec::new();
        for k in KNOWN {
            let found = match env.os {
                Os::Windows => Env::first_file(k.win.iter().map(|rel| env.local.join(rel))),
                // `/usr/bin/<name>` may be an unrelated program with the same name (Ubuntu's
                // `orca` is the screen reader), so a Linux path counts only if it is Electron.
                _ => k.linux.iter().map(PathBuf::from).find(|p| p.is_file() && is_electron_program(p)),
            };
            if let Some(exe) = found {
                out.push(electron_install(format!("electron:{}", k.id), k.name, exe, k.data_dir, k.logo, k.color));
            }
        }
        out
    }

    fn recognize(&self, exe: &Path, _env: &Env) -> Option<Installation> {
        if !looks_like_electron(exe) {
            return None;
        }
        let stem = exe.file_stem()?.to_string_lossy().to_string();
        let mut i = electron_install(
            format!("electron:{}", crate::model::new_id()),
            &stem,
            exe.to_path_buf(),
            &stem,
            "generic",
            "#8E8E93",
        );
        i.source = InstallSource::Manual;
        Some(i)
    }

    fn new_profile_dir(&self, install: &Installation, slug: &str, env: &Env) -> PathBuf {
        let base = opt_str(install, "dataDirName").unwrap_or(&install.name);
        env.config.join(format!("{base}-{slug}"))
    }

    fn default_profile_dir(&self, install: &Installation, env: &Env) -> Option<PathBuf> {
        opt_str(install, "dataDirName").map(|d| env.config.join(d))
    }

    fn launch_plan(&self, install: &Installation, instance: &Instance, _ctx: &Ctx) -> Result<LaunchPlan, String> {
        let mut args = user_data_dir_args(instance);
        args.extend(instance.launch.args.iter().cloned());
        Ok(LaunchPlan { program: install.exe.clone(), args, env: instance.launch.env.clone(), cwd: install.exe.parent().map(Path::to_path_buf) })
    }

    fn matcher(&self, install: &Installation, instance: &Instance, ctx: &Ctx) -> ProcessMatcher {
        // An AppImage (`cursor.appimage`) runs as the program inside it (`cursor`).
        let name = install.exe.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let stem = install.exe.file_stem().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        data_dir_matcher(&[name.as_str(), stem.as_str()], instance, ctx.env)
    }

    fn brand(&self, install: &Installation) -> Brand {
        Brand {
            color: opt_str(install, "color").unwrap_or("#8E8E93").into(),
            letter: install.name.chars().next().unwrap_or('A').to_uppercase().to_string(),
            logo: opt_str(install, "logo").unwrap_or("generic").into(),
        }
    }
}
