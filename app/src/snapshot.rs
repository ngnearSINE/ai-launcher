//! The single state document every window renders from.

use launcher_core::adapters::webapp::PRESETS;
use launcher_core::catalog::effective;
use launcher_core::{Brand, Group, Installation, Instance, Isolation, Settings};
use serde::Serialize;

use crate::core::Core;
use crate::usage::UsageEntry;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub version: &'static str,
    pub os: &'static str,
    pub settings: Settings,
    pub groups: Vec<Group>,
    pub installations: Vec<InstallView>,
    pub instances: Vec<InstanceView>,
    pub presets: Vec<PresetView>,
    pub config_dir: String,
    pub update: crate::updater::UpdateState,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallView {
    #[serde(flatten)]
    pub install: Installation,
    pub brand: Brand,
    pub isolation: Isolation,
    pub has_usage: bool,
    pub supports_default_profile: bool,
    pub default_profile_dir: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectiveView {
    pub taskbar_separate: bool,
    pub taskbar_badge: bool,
    pub relaunch_through_launcher: bool,
    pub usage_enabled: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceView {
    #[serde(flatten)]
    pub instance: Instance,
    pub effective: EffectiveView,
    pub running: bool,
    pub pids: Vec<u32>,
    pub profile_dir: Option<String>,
    pub profile_exists: bool,
    pub usage: Option<UsageEntry>,
    pub app_id: String,
    /// Separation was turned off but this app can only rejoin its group after a restart.
    pub taskbar_needs_restart: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetView {
    pub id: &'static str,
    pub name: &'static str,
    pub url: &'static str,
    pub logo: &'static str,
    pub color: &'static str,
}

pub fn build(core: &Core) -> Snapshot {
    let store = core.store.lock();
    let data = &store.data;
    let runtime = core.runtime.read();
    let usage = core.usage.lock();
    #[cfg(windows)]
    let restart = crate::taskbar::needs_restart();
    #[cfg(not(windows))]
    let restart: std::collections::HashSet<String> = Default::default();

    let installations = data
        .installations
        .iter()
        .map(|i| {
            let a = core.registry.get(&i.adapter);
            InstallView {
                brand: a.map(|a| a.brand(i)).unwrap_or(Brand { color: "#8E8E93".into(), letter: "?".into(), logo: "generic".into() }),
                isolation: a.map(|a| a.isolation(i)).unwrap_or(Isolation::None),
                has_usage: a.and_then(|a| a.usage(i)).is_some(),
                supports_default_profile: a.map(|a| a.supports_default_profile(i)).unwrap_or(false),
                default_profile_dir: a.and_then(|a| a.default_profile_dir(i, &core.env)).map(|p| p.display().to_string()),
                install: i.clone(),
            }
        })
        .collect();

    let mut instances: Vec<InstanceView> = data
        .instances
        .iter()
        .map(|inst| {
            let eff = effective(&data.settings, inst);
            let run = runtime.by_instance.get(&inst.id);
            let dir = launcher_core::catalog::physical_profile_dir(data, &core.registry, &core.env, inst);
            InstanceView {
                effective: EffectiveView {
                    taskbar_separate: eff.taskbar_separate,
                    taskbar_badge: eff.taskbar_badge,
                    relaunch_through_launcher: eff.relaunch_through_launcher,
                    usage_enabled: eff.usage_enabled,
                },
                running: run.is_some(),
                pids: run.map(|r| r.main_pids.clone()).unwrap_or_default(),
                profile_exists: dir.as_ref().map(|d| d.exists()).unwrap_or(false),
                profile_dir: dir.map(|d| d.display().to_string()),
                usage: usage.get(&inst.id).cloned(),
                app_id: format!("AiLauncher.{}", inst.id),
                taskbar_needs_restart: restart.contains(&inst.id),
                instance: inst.clone(),
            }
        })
        .collect();
    instances.sort_by_key(|i| i.instance.order);

    Snapshot {
        version: env!("CARGO_PKG_VERSION"),
        os: match core.env.os {
            launcher_core::env::Os::Windows => "windows",
            launcher_core::env::Os::Linux => "linux",
            launcher_core::env::Os::Mac => "mac",
        },
        settings: data.settings.clone(),
        groups: data.groups.clone(),
        installations,
        instances,
        presets: PRESETS.iter().map(|p| PresetView { id: p.id, name: p.name, url: p.url, logo: p.logo, color: p.color }).collect(),
        config_dir: core.config_dir.display().to_string(),
        update: core.update.lock().clone(),
    }
}
