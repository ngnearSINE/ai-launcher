use std::path::PathBuf;

use super::chromium::process_names;
use super::{detected, opt_str};
use crate::adapter::*;
use crate::env::{Env, Os};
use crate::model::{Installation, Instance};

/// A web service opened as its own app window through a Chromium browser.
/// Every instance owns a separate browser data folder, so each keeps its own sign-in and
/// shows up as its own window/taskbar entry.
pub struct WebAppAdapter;

pub struct Preset {
    pub id: &'static str,
    pub name: &'static str,
    pub url: &'static str,
    pub logo: &'static str,
    pub color: &'static str,
    /// Created automatically when a browser is found (others are offered in "Add app").
    pub auto: bool,
}

pub const PRESETS: &[Preset] = &[
    Preset { id: "gemini", name: "Gemini", url: "https://gemini.google.com/app", logo: "gemini", color: "#4E82EE", auto: true },
    Preset { id: "grok", name: "Grok", url: "https://grok.com", logo: "grok", color: "#A1A1AA", auto: true },
    Preset { id: "chatgpt", name: "ChatGPT", url: "https://chatgpt.com", logo: "chatgpt", color: "#10A37F", auto: false },
    Preset { id: "claude-web", name: "Claude (web)", url: "https://claude.ai", logo: "claude", color: "#D97757", auto: false },
    Preset { id: "perplexity", name: "Perplexity", url: "https://www.perplexity.ai", logo: "perplexity", color: "#20808D", auto: false },
    Preset { id: "ai-studio", name: "Google AI Studio", url: "https://aistudio.google.com", logo: "gemini", color: "#4E82EE", auto: false },
];

/// A web-app installation bound to a browser installation.
pub fn webapp_install(id: &str, name: &str, url: &str, logo: &str, color: &str, browser: &Installation) -> Installation {
    let mut i = detected(id, "webapp", name, browser.exe.clone());
    i.options.insert("browserInstall".into(), browser.id.clone().into());
    i.options.insert("url".into(), url.into());
    i.options.insert("logo".into(), logo.into());
    i.options.insert("color".into(), color.into());
    i
}

pub fn preset_install(p: &Preset, browser: &Installation) -> Installation {
    webapp_install(&format!("webapp:{}", p.id), p.name, p.url, p.logo, p.color, browser)
}

impl WebAppAdapter {
    fn browser<'a>(&self, install: &Installation, ctx: &'a Ctx) -> Option<&'a Installation> {
        opt_str(install, "browserInstall").and_then(|id| ctx.installation(id))
    }
}

impl AppAdapter for WebAppAdapter {
    fn kind(&self) -> &'static str {
        "webapp"
    }

    /// Web apps are derived from browsers by the catalog, not found on disk.
    fn detect(&self, _env: &Env) -> Vec<Installation> {
        Vec::new()
    }

    fn supports_default_profile(&self, _install: &Installation) -> bool {
        false
    }

    fn new_profile_dir(&self, install: &Installation, slug: &str, env: &Env) -> PathBuf {
        env.launcher_data.join("profiles").join(format!("{}-{}", install_slug(install), slug.to_lowercase()))
    }

    fn launch_plan(&self, install: &Installation, instance: &Instance, ctx: &Ctx) -> Result<LaunchPlan, String> {
        let browser = self.browser(install, ctx).ok_or("browser-missing")?;
        let url = instance.launch.url.as_deref().or(opt_str(install, "url")).ok_or("url-missing")?;
        let mut args = user_data_dir_args(instance);
        args.push(format!("--app={url}"));
        args.push("--no-first-run".into());
        args.push("--no-default-browser-check".into());
        if ctx.env.os == Os::Linux {
            // Lets the dock tell instances apart and match a per-instance .desktop entry.
            args.push(format!("--class=AiLauncher-{}", instance.id));
        }
        args.extend(instance.launch.args.iter().cloned());
        Ok(LaunchPlan { program: browser.exe.clone(), args, env: instance.launch.env.clone(), cwd: None })
    }

    fn matcher(&self, install: &Installation, instance: &Instance, ctx: &Ctx) -> ProcessMatcher {
        let names = self.browser(install, ctx).map(|b| process_names(b, ctx.env)).unwrap_or_else(|| vec!["chrome.exe".into()]);
        let mut m = data_dir_matcher(&[], instance, ctx.env);
        m.exe_names = names;
        m
    }

    fn brand(&self, install: &Installation) -> Brand {
        Brand {
            color: opt_str(install, "color").unwrap_or("#8E8E93").into(),
            letter: install.name.chars().next().unwrap_or('W').to_string(),
            logo: opt_str(install, "logo").unwrap_or("web").into(),
        }
    }
}
