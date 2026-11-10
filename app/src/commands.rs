//! Commands the UI invokes. Each one goes through `Core` so saving, broadcasting and tray
//! updates happen in one place.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use launcher_core::adapters::webapp::{preset_install, webapp_install, PRESETS};
use launcher_core::catalog;
use launcher_core::{new_id, Group, Installation, Instance, Settings};
use serde_json::Value;
use tauri::{AppHandle, Manager, State};

use crate::core::Core;
use crate::snapshot::{self, Snapshot};

type Res<T> = Result<T, String>;
type C<'a> = State<'a, Arc<Core>>;

pub fn handler() -> impl Fn(tauri::ipc::Invoke) -> bool + Send + Sync + 'static {
    tauri::generate_handler![
        get_state,
        launch,
        focus,
        close_windows,
        quit,
        restart,
        create_instance,
        update_instance,
        duplicate_instance,
        delete_instance,
        reorder_instances,
        create_group,
        rename_group,
        delete_group,
        add_installation,
        add_webapp,
        update_installation,
        remove_installation,
        rescan,
        update_settings,
        reset_settings,
        refresh_usage,
        open_profile,
        open_config_dir,
        create_shortcut,
        profile_size,
        instance_stats,
        export_catalog,
        import_catalog,
        show_window,
        hide_window,
        set_float,
        launch_best_claude,
        quit_app,
        app_icons,
        check_update,
        install_update,
        add_installed_web_app,
        switch_view,
        fit_float,
    ]
}

/// The floating launcher's size for its content, kept on screen (logical px).
#[tauri::command]
async fn fit_float(app: AppHandle, width: f64, height: f64) -> Res<()> {
    if let Some(w) = app.get_webview_window("float") {
        crate::windows::fit_float(&w, width, height);
    }
    Ok(())
}

/// `to`: `float` or `main`. Async: it may create the floating window.
#[tauri::command]
async fn switch_view(app: AppHandle, core: State<'_, Arc<Core>>, to: String) -> Res<()> {
    log::info!("switch view requested by the UI");
    crate::windows::switch_view(&app, core.inner(), to == "float");
    Ok(())
}

/// Adds an installed browser web app (from the Start menu) with its default instance.
#[tauri::command]
fn add_installed_web_app(core: C, app_id: String) -> Res<String> {
    let p = core.web_apps.lock().iter().find(|p| p.app_id == app_id).cloned().ok_or("not-a-file")?;
    core.mutate(true, |d| {
        let browsers: Vec<Installation> = d.installations.iter().filter(|i| i.adapter == "chromium").cloned().collect();
        let install = launcher_core::adapters::webapp::pwa_install(&p, &browsers, &core.env);
        let id = install.id.clone();
        d.dismissed.retain(|x| x != &id);
        if !d.installations.iter().any(|i| i.id == id) {
            let alias = install.name.clone();
            d.installations.push(install);
            catalog::create_instance(d, &core.registry, &core.env, &id, &alias, None, true)?;
        }
        Ok(id)
    })
}

#[tauri::command]
async fn check_update(core: State<'_, Arc<Core>>) -> Res<()> {
    let core = core.inner().clone();
    // A manual check clears a previous error or "latest" so it runs again.
    {
        let mut s = core.update.lock();
        if matches!(*s, crate::updater::UpdateState::Error { .. } | crate::updater::UpdateState::Latest { .. }) {
            *s = crate::updater::UpdateState::Idle;
        }
    }
    crate::updater::check(&core, true).await;
    Ok(())
}

#[tauri::command]
async fn install_update(core: State<'_, Arc<Core>>) -> Res<bool> {
    Ok(crate::updater::install(core.inner()).await)
}

#[tauri::command]
async fn app_icons(core: State<'_, Arc<Core>>) -> Res<std::collections::HashMap<String, String>> {
    let core = core.inner().clone();
    Ok(tauri::async_runtime::spawn_blocking(move || crate::icons::app_icons(&core)).await.unwrap_or_default())
}

#[tauri::command]
fn get_state(core: C) -> Snapshot {
    snapshot::build(&core)
}

#[tauri::command]
fn launch(core: C, id: String) -> Res<()> {
    core.launch(&id)
}

#[tauri::command]
fn focus(core: C, id: String) -> bool {
    core.focus(&id)
}

#[tauri::command]
fn close_windows(core: C, id: String) -> bool {
    core.close_windows(&id)
}

#[tauri::command]
fn quit(core: C, id: String) -> usize {
    core.quit(&id)
}

#[tauri::command]
fn restart(core: C, id: String) -> Res<()> {
    core.restart(&id)
}

#[tauri::command]
fn create_instance(core: C, installation_id: String, alias: String, color: Option<String>, group_id: Option<String>, use_default: Option<bool>, launch_now: Option<bool>) -> Res<String> {
    let inst = core.mutate(true, |d| {
        let mut inst = catalog::create_instance(d, &core.registry, &core.env, &installation_id, &alias, color, use_default.unwrap_or(false))?;
        inst.group_id = group_id.filter(|g| d.groups.iter().any(|x| &x.id == g));
        if let Some(slot) = d.instances.iter_mut().find(|i| i.id == inst.id) {
            *slot = inst.clone();
        }
        Ok(inst)
    })?;
    if let Some(dir) = inst.profile.dir() {
        let _ = std::fs::create_dir_all(dir);
    }
    if launch_now.unwrap_or(false) {
        core.launch(&inst.id)?;
    }
    Ok(inst.id)
}

/// Fields the UI may change on an instance.
const INSTANCE_FIELDS: &[&str] = &["alias", "color", "glyph", "groupId", "note", "launch", "advanced", "favorite"];

fn merge_fields(target: &mut Value, patch: &Value, allowed: &[&str]) {
    if let (Some(t), Some(p)) = (target.as_object_mut(), patch.as_object()) {
        for (k, v) in p {
            if allowed.contains(&k.as_str()) {
                t.insert(k.clone(), v.clone());
            }
        }
    }
}

#[tauri::command]
fn update_instance(core: C, id: String, patch: Value) -> Res<()> {
    core.mutate(true, |d| {
        let slot = d.instances.iter_mut().find(|i| i.id == id).ok_or("instance-missing")?;
        let mut v = serde_json::to_value(&*slot).map_err(|e| e.to_string())?;
        merge_fields(&mut v, &patch, INSTANCE_FIELDS);
        let mut next: Instance = serde_json::from_value(v).map_err(|e| e.to_string())?;
        if next.alias.trim().is_empty() {
            next.alias = slot.alias.clone();
        }
        *slot = next;
        Ok(())
    })?;
    #[cfg(windows)]
    crate::taskbar::invalidate();
    Ok(())
}

#[tauri::command]
fn duplicate_instance(core: C, id: String) -> Res<String> {
    core.mutate(true, |d| catalog::duplicate_instance(d, &core.registry, &core.env, &id).map(|i| i.id))
}

/// Removes an instance. With `delete_data`, its own profile folders go to the recycle bin
/// (or are deleted when the user turned the recycle bin off). The app's default folder is never
/// touched.
#[tauri::command]
fn delete_instance(core: C, id: String, delete_data: bool) -> Res<Vec<String>> {
    let inst = core.instance(&id).ok_or("instance-missing")?;
    if core.run_info(&id).is_some() && delete_data {
        return Err("instance-running".into());
    }
    let paths: Vec<PathBuf> = {
        let store = core.store.lock();
        catalog::instance_data_paths(&store.data, &core.registry, &core.env, &inst)
    };
    let trash = core.settings().delete_moves_to_trash;
    let mut removed = Vec::new();
    if delete_data {
        for p in &paths {
            let r = if trash { trash::delete(p).map_err(|e| e.to_string()) } else { std::fs::remove_dir_all(p).map_err(|e| e.to_string()) };
            r.map_err(|e| format!("delete {}: {e}", p.display()))?;
            removed.push(p.display().to_string());
        }
    }
    core.mutate(true, |d| {
        d.instances.retain(|i| i.id != id);
        if !delete_data {
            catalog::forget_profile(d, &core.env, &inst);
        }
        Ok(())
    })?;
    let _ = std::fs::remove_file(crate::shortcuts::start_menu_path(&core, &inst));
    Ok(removed)
}

#[tauri::command]
fn reorder_instances(core: C, ids: Vec<String>) -> Res<()> {
    core.mutate(true, |d| {
        for (n, id) in ids.iter().enumerate() {
            if let Some(i) = d.instances.iter_mut().find(|i| &i.id == id) {
                i.order = n as i64;
            }
        }
        Ok(())
    })
}

#[tauri::command]
fn create_group(core: C, name: String) -> Res<String> {
    core.mutate(true, |d| {
        let g = Group { id: new_id(), name: name.trim().to_string(), order: d.groups.len() as i64 };
        let id = g.id.clone();
        d.groups.push(g);
        Ok(id)
    })
}

#[tauri::command]
fn rename_group(core: C, id: String, name: String) -> Res<()> {
    core.mutate(true, |d| {
        let g = d.groups.iter_mut().find(|g| g.id == id).ok_or("group-missing")?;
        g.name = name.trim().to_string();
        Ok(())
    })
}

#[tauri::command]
fn delete_group(core: C, id: String) -> Res<()> {
    core.mutate(true, |d| {
        d.groups.retain(|g| g.id != id);
        for i in d.instances.iter_mut().filter(|i| i.group_id.as_deref() == Some(&id)) {
            i.group_id = None;
        }
        Ok(())
    })
}

/// Registers an executable the user picked. Returns the installation id and creates a first
/// instance for it.
#[tauri::command]
fn add_installation(core: C, path: String, name: Option<String>) -> Res<String> {
    let exe = PathBuf::from(path.trim().trim_matches('"'));
    if !exe.is_file() && std::fs::symlink_metadata(&exe).is_err() {
        return Err("not-a-file".into());
    }
    let mut install: Installation = core.registry.recognize(&exe, &core.env).ok_or("unrecognized")?;
    if let Some(n) = name.filter(|n| !n.trim().is_empty()) {
        install.name = n.trim().to_string();
    }
    let id = install.id.clone();
    core.mutate(true, |d| {
        d.installations.push(install.clone());
        let alias = install.name.clone();
        catalog::create_instance(d, &core.registry, &core.env, &id, &alias, None, true)?;
        Ok(())
    })?;
    Ok(id)
}

/// Adds a web app from a preset id, or a custom `{name, url}` served by a browser installation.
#[tauri::command]
fn add_webapp(core: C, preset: Option<String>, name: Option<String>, url: Option<String>, browser: Option<String>) -> Res<String> {
    core.mutate(true, |d| {
        let browser = match &browser {
            Some(b) => d.installations.iter().find(|i| &i.id == b && i.adapter == "chromium"),
            None => d.installations.iter().find(|i| i.adapter == "chromium" && i.present),
        }
        .cloned()
        .ok_or("browser-missing")?;
        let install = match preset.as_deref().and_then(|p| PRESETS.iter().find(|x| x.id == p)) {
            Some(p) => {
                let mut i = preset_install(p, &browser);
                if d.installations.iter().any(|x| x.id == i.id) {
                    i.id = format!("webapp:{}", new_id());
                }
                i
            }
            None => {
                let url = url.clone().filter(|u| u.starts_with("https://") || u.starts_with("http://")).ok_or("url-invalid")?;
                let name = name.clone().filter(|n| !n.trim().is_empty()).ok_or("name-missing")?;
                let mut i = webapp_install(&format!("webapp:{}", new_id()), name.trim(), &url, "web", "#8E8E93", &browser);
                i.source = launcher_core::InstallSource::Manual;
                i
            }
        };
        d.dismissed.retain(|x| x != &install.id);
        let id = install.id.clone();
        let alias = install.name.clone();
        d.installations.push(install);
        catalog::create_instance(d, &core.registry, &core.env, &id, &alias, None, false)?;
        Ok(id)
    })
}

const INSTALL_FIELDS: &[&str] = &["name", "hidden", "options"];

#[tauri::command]
fn update_installation(core: C, id: String, patch: Value) -> Res<()> {
    core.mutate(true, |d| {
        let slot = d.installations.iter_mut().find(|i| i.id == id).ok_or("installation-missing")?;
        let mut v = serde_json::to_value(&*slot).map_err(|e| e.to_string())?;
        // Options merge key by key so the UI can flip one flag without resending the rest.
        if let Some(opts) = patch.get("options").and_then(Value::as_object) {
            if let Some(cur) = v.get_mut("options").and_then(Value::as_object_mut) {
                for (k, val) in opts {
                    cur.insert(k.clone(), val.clone());
                }
            }
        }
        let mut rest = patch.clone();
        if let Some(o) = rest.as_object_mut() {
            o.remove("options");
        }
        merge_fields(&mut v, &rest, INSTALL_FIELDS);
        *slot = serde_json::from_value(v).map_err(|e| e.to_string())?;
        Ok(())
    })
}

/// Removes an app from the launcher (and its instances when `with_instances`). Detected apps are
/// remembered as dismissed so a rescan does not bring them back.
#[tauri::command]
fn remove_installation(core: C, id: String, with_instances: bool) -> Res<()> {
    core.mutate(true, |d| {
        let has = d.instances.iter().any(|i| i.installation_id == id);
        if has && !with_instances {
            return Err("has-instances".into());
        }
        d.instances.retain(|i| i.installation_id != id);
        if let Some(i) = d.installations.iter().find(|i| i.id == id) {
            if i.source == launcher_core::InstallSource::Detected {
                d.dismissed.push(id.clone());
            }
        }
        d.installations.retain(|i| i.id != id);
        Ok(())
    })
}

#[tauri::command]
fn rescan(core: C) -> Res<()> {
    core.mutate(true, |d| {
        d.dismissed.clear();
        Ok(())
    })?;
    core.sync_catalog();
    Ok(())
}

// Commands that may create windows are async: creating a window from a synchronous command
// deadlocks on Windows (the IPC handler holds the main thread).
#[tauri::command]
async fn update_settings(app: AppHandle, core: State<'_, Arc<Core>>, patch: Value) -> Res<()> {
    core.mutate(true, |d| {
        let mut v = serde_json::to_value(&d.settings).map_err(|e| e.to_string())?;
        if let (Some(t), Some(p)) = (v.as_object_mut(), patch.as_object()) {
            for (k, val) in p {
                if t.contains_key(k) {
                    t.insert(k.clone(), val.clone());
                }
            }
        }
        d.settings = serde_json::from_value::<Settings>(v).map_err(|e| e.to_string())?;
        Ok(())
    })?;
    let keys: Vec<&str> = patch.as_object().map(|o| o.keys().map(String::as_str).collect()).unwrap_or_default();
    if keys.iter().any(|k| k.contains("otkey")) {
        crate::hotkeys::apply(&app, &core);
    }
    if keys.iter().any(|k| k.starts_with("float")) {
        crate::windows::sync_float(&app, &core);
    }
    if keys.iter().any(|k| *k == "glass" || *k == "theme" || *k == "backgroundEffect") {
        crate::windows::apply_appearance(&app, &core);
    }
    if keys.contains(&"startWithOs") {
        core.sync_autostart();
    }
    if keys.iter().any(|k| k.starts_with("taskbar")) {
        #[cfg(windows)]
        crate::taskbar::invalidate();
    }
    if keys.iter().any(|k| k.starts_with("usage")) {
        crate::usage::refresh_now(&core, None);
    }
    Ok(())
}

#[tauri::command]
async fn reset_settings(app: AppHandle, core: State<'_, Arc<Core>>) -> Res<()> {
    core.mutate(true, |d| {
        d.settings = Settings::default();
        Ok(())
    })?;
    crate::hotkeys::apply(&app, &core);
    crate::windows::sync_float(&app, &core);
    crate::windows::apply_appearance(&app, &core);
    core.sync_autostart();
    Ok(())
}

#[tauri::command]
fn refresh_usage(core: C, id: Option<String>) {
    crate::usage::refresh_now(&core, id.as_deref());
}

fn open(app: &AppHandle, path: &Path) -> Res<()> {
    use tauri_plugin_opener::OpenerExt;
    app.opener().open_path(path.display().to_string(), None::<&str>).map_err(|e| e.to_string())
}

#[tauri::command]
fn open_profile(app: AppHandle, core: C, id: String) -> Res<()> {
    let inst = core.instance(&id).ok_or("instance-missing")?;
    let dir = core.profile_dir_of(&inst).ok_or("no-profile")?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    open(&app, &dir)
}

#[tauri::command]
fn open_config_dir(app: AppHandle, core: C) -> Res<()> {
    std::fs::create_dir_all(&core.config_dir).map_err(|e| e.to_string())?;
    open(&app, &core.config_dir)
}

/// `place`: `desktop` or `startMenu`. The shortcut carries the instance's taskbar ID, so pinning
/// it and the running window share one button.
#[tauri::command]
fn create_shortcut(core: C, id: String, place: String) -> Res<String> {
    crate::shortcuts::create(&core, &id, &place).map(|p| p.display().to_string())
}

/// Size of the instance's own folders in bytes (default profiles report the app folder).
#[tauri::command]
async fn profile_size(core: State<'_, Arc<Core>>, id: String) -> Res<u64> {
    let inst = core.instance(&id).ok_or("instance-missing")?;
    let dir = core.profile_dir_of(&inst).ok_or("no-profile")?;
    Ok(tauri::async_runtime::spawn_blocking(move || dir_size(&dir)).await.unwrap_or(0))
}

#[derive(serde::Serialize)]
struct InstanceStats {
    memory: u64,
    processes: usize,
}

/// Memory of a running instance (all its processes). Polled by the detail pane only while it
/// shows that instance.
#[tauri::command]
async fn instance_stats(core: State<'_, Arc<Core>>, id: String) -> Res<InstanceStats> {
    let pids = core.run_info(&id).map(|r| r.pids).unwrap_or_default();
    let (memory, processes) = tauri::async_runtime::spawn_blocking(move || launcher_platform::process::memory_of(&pids)).await.map_err(|e| e.to_string())?;
    Ok(InstanceStats { memory, processes })
}

fn dir_size(p: &Path) -> u64 {
    let Ok(rd) = std::fs::read_dir(p) else { return 0 };
    rd.flatten()
        .map(|e| match e.file_type() {
            Ok(t) if t.is_dir() => dir_size(&e.path()),
            Ok(t) if t.is_file() => e.metadata().map(|m| m.len()).unwrap_or(0),
            _ => 0,
        })
        .sum()
}

#[tauri::command]
fn export_catalog(core: C, path: String) -> Res<()> {
    let json = serde_json::to_vec_pretty(&core.store.lock().data).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())
}

/// Replaces settings, groups and instance definitions from an exported file. Installations are
/// re-detected on this machine; instances whose app is missing stay listed as unavailable.
#[tauri::command]
fn import_catalog(core: C, path: String) -> Res<()> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let incoming: launcher_core::store::StoreFile = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    core.mutate(true, |d| {
        d.settings = incoming.settings;
        d.groups = incoming.groups;
        for inst in incoming.instances {
            if !d.instances.iter().any(|i| i.id == inst.id) {
                d.instances.push(inst);
            }
        }
        for inst in incoming.installations {
            if !d.installations.iter().any(|i| i.id == inst.id) {
                d.installations.push(inst);
            }
        }
        Ok(())
    })?;
    core.sync_catalog();
    Ok(())
}

#[tauri::command]
async fn show_window(app: AppHandle, label: String) {
    match label.as_str() {
        "quick" => crate::windows::show_quick(&app, None),
        _ => crate::windows::show_main(&app),
    }
}

#[tauri::command]
fn hide_window(app: AppHandle, label: String) {
    crate::windows::hide(&app, &label);
}

#[tauri::command]
async fn set_float(app: AppHandle, core: State<'_, Arc<Core>>, on: bool) -> Res<()> {
    core.mutate(true, |d| {
        d.settings.float_enabled = on;
        Ok(())
    })?;
    crate::windows::sync_float(&app, &core);
    Ok(())
}

#[tauri::command]
fn launch_best_claude(core: C) -> Res<String> {
    core.launch_best_claude()
}

#[tauri::command]
fn quit_app(app: AppHandle) {
    app.exit(0);
}
