use std::path::PathBuf;

use super::chromium::{process_names, BROWSERS};
use super::{detected, opt_str};
use crate::adapter::*;
use crate::env::{normalize_path, Env, Os, PwaShortcut};
use crate::model::{Installation, Instance};

/// A web service run as its own app window through a Chromium browser.
///
/// Two kinds of copy:
/// - the **installed web app** (PWA) the user already has, started exactly like its Start-menu
///   shortcut (`chrome_proxy --profile-directory=… --app-id=…`) — the default instance, keeping
///   the browser profile's sign-in;
/// - **extra accounts**, each in its own browser data folder opened in app mode on the service
///   URL, so sign-ins never mix and each window gets its own taskbar button.
pub struct WebAppAdapter;

pub struct Preset {
    pub id: &'static str,
    pub name: &'static str,
    pub url: &'static str,
    pub logo: &'static str,
    pub color: &'static str,
}

pub const PRESETS: &[Preset] = &[
    Preset { id: "gemini", name: "Gemini", url: "https://gemini.google.com/app", logo: "gemini", color: "#4E82EE" },
    Preset { id: "grok", name: "Grok", url: "https://grok.com", logo: "grok", color: "#A1A1AA" },
    Preset { id: "chatgpt", name: "ChatGPT", url: "https://chatgpt.com", logo: "chatgpt", color: "#10A37F" },
    Preset { id: "claude-web", name: "Claude", url: "https://claude.ai", logo: "claude", color: "#D97757" },
    Preset { id: "perplexity", name: "Perplexity", url: "https://www.perplexity.ai", logo: "perplexity", color: "#20808D" },
    Preset { id: "copilot", name: "Copilot", url: "https://copilot.microsoft.com", logo: "copilot", color: "#2E8BFF" },
    Preset { id: "ai-studio", name: "Google AI Studio", url: "https://aistudio.google.com", logo: "gemini", color: "#4E82EE" },
    Preset { id: "notebooklm", name: "NotebookLM", url: "https://notebooklm.google.com", logo: "notebooklm", color: "#1A73E8" },
    Preset { id: "deepseek", name: "DeepSeek", url: "https://chat.deepseek.com", logo: "deepseek", color: "#4D6BFE" },
    Preset { id: "lechat", name: "Le Chat", url: "https://chat.mistral.ai", logo: "mistral", color: "#FA520F" },
];

/// Installed web apps registered automatically: AI services. Claude's web app is left out
/// because the desktop app covers it; everything else can be added from "Add app".
pub fn is_auto_ai(name: &str) -> bool {
    let n = name.to_lowercase();
    const AI: &[&str] = &["gemini", "grok", "chatgpt", "perplexity", "copilot", "ai studio", "notebooklm", "deepseek", "le chat", "mistral", "open webui", "poe"];
    AI.iter().any(|k| n.contains(k)) && !n.starts_with("claude")
}

fn preset_for(name: &str) -> Option<&'static Preset> {
    let n = name.to_lowercase();
    PRESETS.iter().find(|p| n.starts_with(&p.name.to_lowercase()))
}

/// A web-app installation bound to a browser installation (extra-account copies only).
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

/// An installed web app (PWA) as an installation. The browser installation is linked when the
/// launcher knows the browser the shortcut belongs to (needed only for extra accounts).
pub fn pwa_install(p: &PwaShortcut, browsers: &[Installation], env: &Env) -> Installation {
    let mut i = detected(&format!("pwa:{}", p.app_id), "webapp", &p.name, p.launcher.clone());
    i.options.insert("pwaAppId".into(), p.app_id.clone().into());
    i.options.insert("pwaProfile".into(), p.profile_directory.clone().into());
    i.options.insert("pwaLauncher".into(), p.launcher.display().to_string().into());
    if let Some(icon) = &p.icon {
        i.options.insert("icon".into(), icon.display().to_string().into());
    }
    let preset = preset_for(&p.name);
    i.options.insert("logo".into(), preset.map(|x| x.logo).unwrap_or("web").into());
    i.options.insert("color".into(), preset.map(|x| x.color).unwrap_or("#8E8E93").into());
    if let Some(pr) = preset {
        i.options.insert("url".into(), pr.url.into());
    }
    // The browser is the one installed next to chrome_proxy/msedge_proxy.
    let dir = p.launcher.parent().map(|d| normalize_path(d, env.os));
    if let Some(b) = browsers.iter().find(|b| b.exe.parent().map(|d| normalize_path(d, env.os)) == dir) {
        i.options.insert("browserInstall".into(), b.id.clone().into());
        i.exe = b.exe.clone();
    }
    i
}

fn is_pwa(install: &Installation) -> bool {
    opt_str(install, "pwaAppId").is_some()
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

    /// Web apps come from the browser's installed apps (see the catalog), not from disk paths.
    fn detect(&self, _env: &Env) -> Vec<Installation> {
        Vec::new()
    }

    fn supports_default_profile(&self, install: &Installation) -> bool {
        is_pwa(install)
    }

    fn new_profile_dir(&self, install: &Installation, slug: &str, env: &Env) -> PathBuf {
        env.launcher_data.join("profiles").join(format!("{}-{}", install_slug(install), slug.to_lowercase()))
    }

    fn launch_plan(&self, install: &Installation, instance: &Instance, ctx: &Ctx) -> Result<LaunchPlan, String> {
        if instance.profile.is_default() {
            let app_id = opt_str(install, "pwaAppId").ok_or("url-missing")?;
            let launcher = opt_str(install, "pwaLauncher").map(PathBuf::from).ok_or("browser-missing")?;
            let mut args = vec![format!("--profile-directory={}", opt_str(install, "pwaProfile").unwrap_or("Default")), format!("--app-id={app_id}")];
            args.extend(instance.launch.args.iter().cloned());
            return Ok(LaunchPlan { program: launcher, args, env: instance.launch.env.clone(), cwd: None });
        }
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

    /// The installed web app runs inside the browser's own process, so it cannot be told apart
    /// from the browser: it never shows as running (launching it again just brings it forward).
    fn matcher(&self, install: &Installation, instance: &Instance, ctx: &Ctx) -> ProcessMatcher {
        if instance.profile.is_default() {
            return ProcessMatcher { exe_names: Vec::new(), exe_path: None, data_dir: None, uses_data_dir: true };
        }
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

/// Browser ids in preference order (used to pick an engine for extra accounts).
pub fn browser_order() -> impl Iterator<Item = &'static str> {
    BROWSERS.iter().map(|b| b.id)
}
