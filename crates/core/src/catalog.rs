//! Keeps the catalog in step with the machine and implements instance bookkeeping.

use std::path::PathBuf;

use chrono::Utc;
use serde::Serialize;

use crate::adapter::{Ctx, Registry};
use crate::adapters::chromium::BROWSERS;
use crate::adapters::webapp::{is_auto_ai, pwa_install, PRESETS};
use crate::env::{normalize_path, Env};
use crate::model::*;
use crate::settings::Settings;
use crate::store::StoreFile;
use crate::text;

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncReport {
    pub added_installations: Vec<String>,
    pub added_instances: Vec<String>,
    pub missing_installations: Vec<String>,
}

/// Detects installations, binds web apps to a browser, creates a first instance for every
/// new installation and adopts profile folders that already exist on disk.
pub fn sync(data: &mut StoreFile, reg: &Registry, env: &Env) -> SyncReport {
    let mut report = SyncReport::default();

    // Profiles assigned under the old data folder (the installer's program folder) that were
    // never created move to the current data folder.
    let old_roots = [env.local.join("AI Launcher").join("profiles"), env.local.join("AI Launcher Data").join("profiles")];
    for inst in data.instances.iter_mut() {
        if let Profile::Dir { path, managed: true } = &mut inst.profile {
            if path.exists() {
                continue;
            }
            if let Some(rest) = old_roots.iter().find_map(|r| crate::env::strip_prefix_ci(path, r)) {
                *path = env.launcher_data.join("profiles").join(rest);
            }
        }
    }

    // 1. Installed apps.
    let detected: Vec<Installation> = reg.all().iter().flat_map(|a| a.detect(env)).collect();
    for found in &detected {
        if data.dismissed.contains(&found.id) {
            continue;
        }
        match data.installations.iter_mut().find(|i| i.id == found.id) {
            Some(existing) => {
                existing.exe = found.exe.clone();
                existing.present = true;
                for (k, v) in &found.options {
                    existing.options.entry(k.clone()).or_insert_with(|| v.clone());
                }
            }
            None => {
                let mut f = found.clone();
                // Browsers are engines for web apps, not apps to show.
                if f.adapter == "chromium" {
                    f.hidden = true;
                }
                data.installations.push(f);
                report.added_installations.push(found.id.clone());
            }
        }
    }
    // Catalogs from before browsers were hidden: hide once, then respect the user's choice.
    for inst in data.installations.iter_mut().filter(|i| i.adapter == "chromium" && i.source == InstallSource::Detected) {
        if !inst.options.contains_key("autoHidden") {
            inst.hidden = true;
            inst.options.insert("autoHidden".into(), true.into());
        }
    }
    for inst in data.installations.iter_mut() {
        let derived = inst.adapter == "webapp";
        if inst.source == InstallSource::Detected && !derived && !detected.iter().any(|d| d.id == inst.id) {
            if inst.present {
                report.missing_installations.push(inst.id.clone());
            }
            inst.present = false;
        }
        if inst.source == InstallSource::Manual {
            inst.present = inst.exe.exists() || std::fs::symlink_metadata(&inst.exe).is_ok();
        }
    }

    // 2. Installed web apps (PWAs). AI services register themselves; others are offered in
    //    "Add app". Web apps an earlier version created from presets are dropped while unused.
    let unused_preset = |id: &str, d: &StoreFile| {
        id.starts_with("webapp:")
            && PRESETS.iter().any(|p| id == format!("webapp:{}", p.id))
            && d.instances.iter().filter(|x| x.installation_id == id).all(|x| x.launch_count == 0)
    };
    let stale: Vec<String> = data
        .installations
        .iter()
        .filter(|i| i.source == InstallSource::Detected && unused_preset(&i.id, data))
        .map(|i| i.id.clone())
        .collect();
    data.instances.retain(|x| !stale.contains(&x.installation_id));
    data.installations.retain(|i| !stale.contains(&i.id));

    let browsers: Vec<Installation> = data.installations.iter().filter(|i| i.adapter == "chromium").cloned().collect();
    for p in &env.pwas {
        let id = format!("pwa:{}", p.app_id);
        match data.installations.iter_mut().find(|i| i.id == id) {
            Some(existing) => {
                let fresh = pwa_install(p, &browsers, env);
                existing.options.extend(fresh.options);
                existing.exe = fresh.exe;
                existing.present = true;
            }
            None if is_auto_ai(&p.name) && !data.dismissed.contains(&id) => {
                data.installations.push(pwa_install(p, &browsers, env));
                report.added_installations.push(id);
            }
            None => {}
        }
    }
    let browser = preferred_browser(data).cloned();
    let present: Vec<String> = data.installations.iter().filter(|i| i.present).map(|i| i.id.clone()).collect();
    for inst in data.installations.iter_mut().filter(|i| i.adapter == "webapp") {
        if let Some(app_id) = inst.options.get("pwaAppId").and_then(|v| v.as_str()) {
            // An installed web app is present while its shortcut is; extra accounts also need a browser.
            inst.present = env.pwas.is_empty() || env.pwas.iter().any(|p| p.app_id == app_id);
            continue;
        }
        let bound = inst.options.get("browserInstall").and_then(|v| v.as_str()).map(str::to_string);
        let ok = bound.as_ref().map(|b| present.contains(b)).unwrap_or(false);
        match (&browser, ok) {
            (_, true) => inst.present = true,
            (Some(b), false) => {
                inst.options.insert("browserInstall".into(), b.id.clone().into());
                inst.exe = b.exe.clone();
                inst.present = true;
            }
            (None, false) => inst.present = false,
        }
    }

    // 3. A first instance for each new installation. Browsers are engines for web apps and get
    //    no card of their own; a browser instance is made on purpose from "New instance".
    //    Earlier versions did create one: such a card is dropped while it was never launched.
    data.instances.retain(|x| {
        let auto_browser = x.profile.is_default()
            && x.launch_count == 0
            && data.installations.iter().any(|i| i.id == x.installation_id && i.adapter == "chromium" && i.source == InstallSource::Detected);
        !auto_browser
    });
    for id in report.added_installations.clone() {
        let Some(install) = data.installations.iter().find(|i| i.id == id).cloned() else { continue };
        if install.adapter == "chromium" || data.instances.iter().any(|x| x.installation_id == id) {
            continue;
        }
        let alias = install.name.clone();
        if let Ok(inst) = create_instance(data, reg, env, &id, &alias, None, true) {
            report.added_instances.push(inst.id);
        }
    }

    // 4. Profile folders that already exist (e.g. made by hand before the launcher).
    let installs: Vec<Installation> = data.installations.iter().filter(|i| i.present).cloned().collect();
    for install in &installs {
        let Some(adapter) = reg.get(&install.adapter) else { continue };
        for (alias, path) in adapter.existing_profiles(install, env) {
            let key = normalize_path(&path, env.os);
            let used = data
                .instances
                .iter()
                .any(|x| x.profile.dir().map(|d| normalize_path(d, env.os)) == Some(key.clone()));
            if used || data.imported_profiles.contains(&key) {
                continue;
            }
            data.imported_profiles.push(key);
            let color = next_color(data);
            let inst = Instance {
                id: new_id(),
                installation_id: install.id.clone(),
                alias,
                color,
                glyph: None,
                profile: Profile::Dir { path, managed: false },
                group_id: None,
                note: String::new(),
                launch: LaunchOptions::default(),
                advanced: InstanceAdvanced::default(),
                order: next_order(data),
                favorite: false,
                created_at: Utc::now(),
                last_launched_at: None,
                launch_count: 0,
            };
            report.added_instances.push(inst.id.clone());
            data.instances.push(inst);
        }
    }
    report
}

fn preferred_browser(data: &StoreFile) -> Option<&Installation> {
    BROWSERS.iter().find_map(|b| {
        data.installations.iter().find(|i| {
            i.adapter == "chromium" && i.present && i.options.get("browser").and_then(|v| v.as_str()) == Some(b.id)
        })
    })
}

pub fn next_color(data: &StoreFile) -> String {
    PALETTE
        .iter()
        .find(|c| !data.instances.iter().any(|i| i.color.eq_ignore_ascii_case(c)))
        .unwrap_or(&PALETTE[data.instances.len() % PALETTE.len()])
        .to_string()
}

fn next_order(data: &StoreFile) -> i64 {
    data.instances.iter().map(|i| i.order).max().unwrap_or(0) + 1
}

/// Adds an instance. `prefer_default` uses the app's own folder when the adapter allows it and
/// no other instance of that installation already does; otherwise a new folder is assigned.
pub fn create_instance(
    data: &mut StoreFile,
    reg: &Registry,
    env: &Env,
    installation_id: &str,
    alias: &str,
    color: Option<String>,
    prefer_default: bool,
) -> Result<Instance, String> {
    let install = data.installations.iter().find(|i| i.id == installation_id).cloned().ok_or("installation-missing")?;
    let adapter = reg.get(&install.adapter).ok_or("adapter-missing")?;
    let id = new_id();
    let default_free = !data.instances.iter().any(|x| x.installation_id == installation_id && x.profile.is_default());
    let isolated = adapter.isolation(&install) == crate::adapter::Isolation::UserDataDir;
    let profile = if !isolated || (prefer_default && default_free && adapter.supports_default_profile(&install)) {
        Profile::Default
    } else {
        // The first copy named after its app gets "main" instead of repeating the app name.
        let base = if alias.trim().eq_ignore_ascii_case(&install.name) { "main".to_string() } else { text::slug(alias, &id) };
        let env = profiles_env(env, &data.settings);
        let taken = |s: &str| {
            let p = adapter.new_profile_dir(&install, s, &env);
            p.exists() || data.instances.iter().any(|x| x.profile.dir() == Some(&p))
        };
        let slug = text::unique(&base, taken);
        Profile::Dir { path: adapter.new_profile_dir(&install, &slug, &env), managed: true }
    };
    let inst = Instance {
        id,
        installation_id: installation_id.to_string(),
        alias: if alias.trim().is_empty() { install.name.clone() } else { alias.trim().to_string() },
        color: color.unwrap_or_else(|| next_color(data)),
        glyph: None,
        profile,
        group_id: None,
        note: String::new(),
        launch: LaunchOptions::default(),
        advanced: InstanceAdvanced::default(),
        order: next_order(data),
        favorite: false,
        created_at: Utc::now(),
        last_launched_at: None,
        launch_count: 0,
    };
    data.instances.push(inst.clone());
    Ok(inst)
}

/// Same installation, same options, a fresh profile folder.
pub fn duplicate_instance(data: &mut StoreFile, reg: &Registry, env: &Env, id: &str) -> Result<Instance, String> {
    let src = data.instances.iter().find(|i| i.id == id).cloned().ok_or("instance-missing")?;
    let mut copy = create_instance(data, reg, env, &src.installation_id, &format!("{} 2", src.alias), None, false)?;
    copy.launch = src.launch.clone();
    copy.advanced = src.advanced.clone();
    copy.group_id = src.group_id.clone();
    if let Some(slot) = data.instances.iter_mut().find(|i| i.id == copy.id) {
        *slot = copy.clone();
    }
    Ok(copy)
}

/// Folders that belong to an instance (its profile and the app's side folders). Empty for
/// the default profile: the launcher never offers to delete an app's own data.
pub fn instance_data_paths(data: &StoreFile, reg: &Registry, env: &Env, inst: &Instance) -> Vec<PathBuf> {
    let Some(dir) = inst.profile.dir() else { return Vec::new() };
    let mut out = vec![dir.clone()];
    if let Some(install) = data.installations.iter().find(|i| i.id == inst.installation_id) {
        if let Some(adapter) = reg.get(&install.adapter) {
            out = adapter.profile_locations(install, dir, env);
            out.extend(adapter.companion_dirs(install, dir, env));
        }
    }
    out.retain(|p| p.exists());
    out.dedup();
    out
}

/// The folder that really holds an instance's data (see `AppAdapter::resolve_profile_dir`),
/// falling back to the app's default folder for default-profile instances.
pub fn physical_profile_dir(data: &StoreFile, reg: &Registry, env: &Env, inst: &Instance) -> Option<PathBuf> {
    let install = data.installations.iter().find(|i| i.id == inst.installation_id)?;
    let adapter = reg.get(&install.adapter)?;
    let logical = match inst.profile.dir() {
        Some(d) => d.clone(),
        None => adapter.default_profile_dir(install, env)?,
    };
    Some(adapter.resolve_profile_dir(install, &logical, env))
}

/// Remember a profile folder so the next scan does not offer it again (instance removed but
/// its data kept).
pub fn forget_profile(data: &mut StoreFile, env: &Env, inst: &Instance) {
    if let Some(dir) = inst.profile.dir() {
        let key = normalize_path(dir, env.os);
        if !data.imported_profiles.contains(&key) {
            data.imported_profiles.push(key);
        }
    }
}

/// Profiles can be placed under a user-chosen root (advanced setting) for apps whose adapter
/// uses the launcher's own folder.
fn profiles_env(env: &Env, settings: &Settings) -> Env {
    let mut e = env.clone();
    if let Some(root) = settings.profiles_root.as_deref().filter(|r| !r.trim().is_empty()) {
        e.launcher_data = PathBuf::from(root);
    }
    e
}

pub fn ctx<'a>(data: &'a StoreFile, env: &'a Env) -> Ctx<'a> {
    Ctx { env, installations: &data.installations }
}

/// Effective per-instance switches (instance override, else global setting).
pub struct Effective {
    pub taskbar_separate: bool,
    pub taskbar_badge: bool,
    pub relaunch_through_launcher: bool,
    pub usage_enabled: bool,
}

pub fn effective(settings: &Settings, inst: &Instance) -> Effective {
    let a = &inst.advanced;
    let separate_default = settings.taskbar_separate && (!inst.profile.is_default() || settings.taskbar_tag_default_instance);
    Effective {
        taskbar_separate: a.taskbar_separate.resolve(separate_default),
        taskbar_badge: a.taskbar_badge.resolve(settings.taskbar_badge),
        relaunch_through_launcher: a.relaunch_through_launcher.resolve(settings.taskbar_relaunch_through_launcher),
        usage_enabled: a.usage_enabled.resolve(settings.usage_enabled),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::env::Os;

    fn env(root: &std::path::Path) -> Env {
        Env {
            os: Os::Windows,
            home: root.into(),
            config: root.join("Roaming"),
            local: root.join("Local"),
            program_files: vec![root.join("PF")],
            path: vec![],
            launcher_data: root.join("Local").join("AI Launcher"),
            pwas: vec![],
        }
    }

    #[test]
    fn sync_creates_default_and_imports_existing() {
        let dir = tempfile::tempdir().unwrap();
        let e = env(dir.path());
        let squirrel = e.local.join("AnthropicClaude");
        std::fs::create_dir_all(&squirrel).unwrap();
        std::fs::write(squirrel.join("claude.exe"), b"").unwrap();
        let extra = e.config.join("Claude-Work");
        std::fs::create_dir_all(&extra).unwrap();
        std::fs::write(extra.join("config.json"), b"{}").unwrap();
        std::fs::create_dir_all(e.config.join("Claude-3p")).unwrap();
        std::fs::write(e.config.join("Claude-3p").join("config.json"), b"{}").unwrap();
        // Squirrel-style Electron app (stub + app-<version>\resources\app.asar).
        let poe = e.local.join("Poe");
        std::fs::create_dir_all(poe.join("app-1.1.40").join("resources")).unwrap();
        std::fs::write(poe.join("Poe.exe"), b"").unwrap();
        std::fs::write(poe.join("app-1.1.40").join("resources").join("app.asar"), b"").unwrap();
        assert!(crate::adapters::electron::looks_like_electron(&poe.join("Poe.exe")));
        let chrome = e.program_files[0].join("Google/Chrome/Application");
        std::fs::create_dir_all(&chrome).unwrap();
        std::fs::write(chrome.join("chrome.exe"), b"").unwrap();

        let mut e = e;
        let pwa = |name: &str, id: &str| crate::env::PwaShortcut {
            name: name.into(),
            launcher: chrome.join("chrome_proxy.exe"),
            profile_directory: "Profile 1".into(),
            app_id: id.into(),
            icon: None,
        };
        e.pwas = vec![pwa("Gemini", "kidh"), pwa("Google Keep", "hmjk")];

        let reg = Registry::standard();
        let mut data = StoreFile::default();
        let r = sync(&mut data, &reg, &e);
        assert!(r.added_installations.contains(&"claude:classic".to_string()));
        assert!(r.added_installations.contains(&"electron:poe".to_string()));
        assert!(r.added_installations.contains(&"pwa:kidh".to_string()), "AI web apps register themselves");
        assert!(!r.added_installations.contains(&"pwa:hmjk".to_string()), "other web apps are only offered");
        assert!(data.installations.iter().find(|i| i.id == "chromium:chrome").unwrap().hidden, "browsers are hidden");
        let claude: Vec<_> = data.instances.iter().filter(|i| i.installation_id == "claude:classic").collect();
        assert_eq!(claude.len(), 2, "default + Claude-Work, not Claude-3p");
        assert!(claude.iter().any(|i| i.profile.is_default()));
        assert!(claude.iter().any(|i| i.alias == "Work"));
        assert!(!data.instances.iter().any(|i| i.installation_id == "chromium:chrome"), "browsers get no automatic card");
        assert!(data.installations.iter().any(|i| i.id == "chromium:chrome"), "but stay available as an installation");
        let gemini = data.instances.iter().find(|i| i.installation_id == "pwa:kidh").unwrap();
        assert!(gemini.profile.is_default(), "the installed web app is the default instance");
        let gi = data.installations.iter().find(|i| i.id == "pwa:kidh").unwrap().clone();
        let c = ctx(&data, &e);
        let plan = reg.get("webapp").unwrap().launch_plan(&gi, gemini, &c).unwrap();
        assert_eq!(plan.args, ["--profile-directory=Profile 1", "--app-id=kidh"]);
        assert!(plan.program.ends_with("chrome_proxy.exe"));
        // An extra account opens the service URL in its own browser folder.
        let mut d2 = data.clone();
        let extra = create_instance(&mut d2, &reg, &e, "pwa:kidh", "Work", None, false).unwrap();
        let c2 = ctx(&d2, &e);
        let p2 = reg.get("webapp").unwrap().launch_plan(&gi, &extra, &c2).unwrap();
        assert!(p2.args.iter().any(|a| a == "--app=https://gemini.google.com/app"));
        assert!(p2.args[0].starts_with("--user-data-dir="));

        // Second sync is stable.
        let r2 = sync(&mut data, &reg, &e);
        assert!(r2.added_instances.is_empty());

        // New instance gets Claude-<slug> next to the default folder.
        let n = create_instance(&mut data, &reg, &e, "claude:classic", "Team A", None, true).unwrap();
        assert_eq!(n.profile.dir().unwrap(), &e.launcher_data.join("profiles").join("Claude-Team-A"));
        let ctx = ctx(&data, &e);
        let plan = reg.get("claude").unwrap().launch_plan(&data.installations[0], &n, &ctx).unwrap();
        assert!(plan.args[0].starts_with("--user-data-dir="));
    }
}
