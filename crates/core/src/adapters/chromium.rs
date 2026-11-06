use std::path::{Path, PathBuf};

use super::{detected, file_name_lower, opt_str};
use crate::adapter::*;
use crate::env::{Env, Os};
use crate::model::{InstallSource, Installation, Instance};

/// Chromium browsers. A browser instance is a separate `--user-data-dir`, which also gives it
/// its own process and taskbar group (a Chrome *profile* inside one data folder would share both).
pub struct ChromiumAdapter;

pub struct Browser {
    pub id: &'static str,
    pub name: &'static str,
    pub logo: &'static str,
    pub color: &'static str,
    /// Paths relative to Program Files / LocalAppData on Windows.
    pub win: &'static [&'static str],
    /// Commands looked up on PATH on Linux.
    pub linux: &'static [&'static str],
    /// Executable names of the running browser process.
    pub process: &'static [&'static str],
    /// Default data folder relative to LocalAppData (Windows) / config (Linux).
    pub win_data: &'static str,
    pub linux_data: &'static str,
}

pub const BROWSERS: &[Browser] = &[
    Browser {
        id: "chrome",
        name: "Google Chrome",
        logo: "chrome",
        color: "#4285F4",
        win: &["Google/Chrome/Application/chrome.exe"],
        linux: &["google-chrome", "google-chrome-stable"],
        process: &["chrome"],
        win_data: "Google/Chrome/User Data",
        linux_data: "google-chrome",
    },
    Browser {
        id: "edge",
        name: "Microsoft Edge",
        logo: "edge",
        color: "#0C8CE9",
        win: &["Microsoft/Edge/Application/msedge.exe"],
        linux: &["microsoft-edge", "microsoft-edge-stable"],
        process: &["msedge"],
        win_data: "Microsoft/Edge/User Data",
        linux_data: "microsoft-edge",
    },
    Browser {
        id: "brave",
        name: "Brave",
        logo: "brave",
        color: "#FB542B",
        win: &["BraveSoftware/Brave-Browser/Application/brave.exe"],
        linux: &["brave-browser"],
        process: &["brave"],
        win_data: "BraveSoftware/Brave-Browser/User Data",
        linux_data: "BraveSoftware/Brave-Browser",
    },
    Browser {
        id: "chromium",
        name: "Chromium",
        logo: "chromium",
        color: "#5B8DEF",
        win: &["Chromium/Application/chrome.exe"],
        linux: &["chromium", "chromium-browser"],
        process: &["chromium", "chrome"],
        win_data: "Chromium/User Data",
        linux_data: "chromium",
    },
];

pub fn browser_of(install: &Installation) -> Option<&'static Browser> {
    let id = opt_str(install, "browser")?;
    BROWSERS.iter().find(|b| b.id == id)
}

/// Process names of the browser behind an installation (Windows adds `.exe`).
pub fn process_names(install: &Installation, env: &Env) -> Vec<String> {
    let names: Vec<&str> = browser_of(install).map(|b| b.process.to_vec()).unwrap_or_else(|| vec!["chrome"]);
    let mut out = super::exe_names(env, &names);
    // Whatever the binary is actually called (e.g. a portable build) also counts.
    let own = file_name_lower(&install.exe);
    if !own.is_empty() && !out.contains(&own) && env.os == Os::Windows {
        out.push(own);
    }
    out
}

fn browser_install(b: &Browser, exe: PathBuf) -> Installation {
    let mut i = detected(&format!("chromium:{}", b.id), "chromium", b.name, exe);
    i.options.insert("browser".into(), b.id.into());
    i
}

impl AppAdapter for ChromiumAdapter {
    fn kind(&self) -> &'static str {
        "chromium"
    }

    fn detect(&self, env: &Env) -> Vec<Installation> {
        BROWSERS
            .iter()
            .filter_map(|b| {
                let exe = match env.os {
                    Os::Windows => {
                        let roots = env.program_files.iter().chain(std::iter::once(&env.local));
                        Env::first_file(roots.flat_map(|r| b.win.iter().map(move |rel| r.join(rel))))
                    }
                    _ => b.linux.iter().find_map(|c| env.which(c)),
                }?;
                Some(browser_install(b, exe))
            })
            .collect()
    }

    fn recognize(&self, exe: &Path, _env: &Env) -> Option<Installation> {
        let name = file_name_lower(exe);
        let b = BROWSERS.iter().find(|b| b.process.iter().any(|p| name == *p || name == format!("{p}.exe")))?;
        let mut i = browser_install(b, exe.to_path_buf());
        i.id = format!("chromium:{}", crate::model::new_id());
        i.source = InstallSource::Manual;
        Some(i)
    }

    fn default_profile_dir(&self, install: &Installation, env: &Env) -> Option<PathBuf> {
        let b = browser_of(install)?;
        Some(match env.os {
            Os::Windows => env.local.join(b.win_data),
            _ => env.config.join(b.linux_data),
        })
    }

    fn launch_plan(&self, install: &Installation, instance: &Instance, _ctx: &Ctx) -> Result<LaunchPlan, String> {
        let mut args = user_data_dir_args(instance);
        if instance.profile.dir().is_some() {
            args.push("--no-first-run".into());
            args.push("--no-default-browser-check".into());
        }
        args.extend(instance.launch.args.iter().cloned());
        Ok(LaunchPlan { program: install.exe.clone(), args, env: instance.launch.env.clone(), cwd: None })
    }

    fn matcher(&self, install: &Installation, instance: &Instance, ctx: &Ctx) -> ProcessMatcher {
        let names = process_names(install, ctx.env);
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        let mut m = data_dir_matcher(&refs, instance, ctx.env);
        m.exe_names = names;
        m
    }

    fn brand(&self, install: &Installation) -> Brand {
        let b = browser_of(install);
        Brand {
            color: b.map(|b| b.color).unwrap_or("#4285F4").into(),
            letter: install.name.chars().next().unwrap_or('B').to_string(),
            logo: b.map(|b| b.logo).unwrap_or("chromium").into(),
        }
    }
}
