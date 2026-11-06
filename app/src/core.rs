//! Application core: owns the catalog, knows what is running, and is the only place that
//! starts or stops instances. Windows, tray and commands call into it; it broadcasts a fresh
//! snapshot to every window whenever something visible changes.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use launcher_core::adapter::Ctx;
use launcher_core::catalog;
use launcher_core::store::StoreFile;
use launcher_core::{Env, Instance, LaunchPlan, Registry, Settings, Store};
use parking_lot::{Mutex, RwLock};
use tauri::{AppHandle, Emitter};

use crate::usage::UsageEntry;

/// What runs for one instance right now.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RunInfo {
    /// Main processes (normally one).
    pub main_pids: Vec<u32>,
    /// Main processes and all their children.
    pub pids: Vec<u32>,
    /// Executable of the main process (used for icons).
    pub exe: Option<PathBuf>,
}

#[derive(Debug, Default)]
pub struct Runtime {
    pub by_instance: HashMap<String, RunInfo>,
    pub pid_index: HashMap<u32, String>,
}

pub struct Core {
    pub app: AppHandle,
    pub env: Env,
    pub registry: Registry,
    pub store: Mutex<Store>,
    pub runtime: RwLock<Runtime>,
    pub usage: Mutex<HashMap<String, UsageEntry>>,
    pub usage_wake: tokio::sync::Notify,
    pub http: launcher_usage::reqwest::Client,
    pub config_dir: PathBuf,
    pub update: Mutex<crate::updater::UpdateState>,
    /// Installed web apps seen at the last scan (offered in "Add app").
    pub web_apps: Mutex<Vec<launcher_core::env::PwaShortcut>>,
    pub update_pending: tokio::sync::Mutex<Option<crate::updater::Pending>>,
    monitor_tx: Mutex<Sender<()>>,
    pub monitor_rx: Mutex<Option<Receiver<()>>>,
    save_gen: AtomicU64,
}

impl Core {
    pub fn new(app: AppHandle) -> Result<Self, String> {
        let env = Env::detect();
        let config_dir = env.config.join("AI Launcher");
        let store = Store::load(&config_dir.join("store.json")).map_err(|e| e.to_string())?;
        let (tx, rx) = channel();
        let http = launcher_usage::reqwest::Client::builder()
            .user_agent(concat!("ai-launcher/", env!("CARGO_PKG_VERSION")))
            .redirect(launcher_usage::reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self {
            app,
            env,
            registry: Registry::standard(),
            store: Mutex::new(store),
            runtime: RwLock::new(Runtime::default()),
            usage: Mutex::new(HashMap::new()),
            usage_wake: tokio::sync::Notify::new(),
            http,
            config_dir,
            update: Mutex::new(crate::updater::UpdateState::Idle),
            web_apps: Mutex::new(Vec::new()),
            update_pending: tokio::sync::Mutex::new(None),
            monitor_tx: Mutex::new(tx),
            monitor_rx: Mutex::new(Some(rx)),
            save_gen: AtomicU64::new(0),
        })
    }

    pub fn settings(&self) -> Settings {
        self.store.lock().data.settings.clone()
    }

    /// Generated icons, outside AppData. Explorer reads them for the taskbar; if the launcher
    /// itself runs inside a Store package's context (e.g. started from a packaged app), its AppData
    /// writes are virtualized and Explorer would see no file — a blank taskbar icon.
    pub fn icons_dir(&self) -> PathBuf {
        self.env.launcher_data.join("icons")
    }

    /// Installed browser web apps, freshly read from the Start menu / `.desktop` entries.
    pub fn scan_web_apps(&self) -> Vec<launcher_core::env::PwaShortcut> {
        let found: Vec<_> = launcher_platform::installed_web_apps()
            .into_iter()
            .filter_map(|(name, launcher, args, icon)| launcher_core::env::PwaShortcut::from_args(&name, launcher, &args, icon))
            .collect();
        *self.web_apps.lock() = found.clone();
        found
    }

    /// Detects apps and adopts existing profiles, then saves and broadcasts.
    pub fn sync_catalog(&self) {
        let mut env = self.env.clone();
        env.pwas = self.scan_web_apps();
        {
            let mut store = self.store.lock();
            let report = catalog::sync(&mut store.data, &self.registry, &env);
            log::info!("catalog sync: {report:?}");
            if let Err(e) = store.save() {
                log::error!("save: {e}");
            }
        }
        self.changed(true);
    }

    /// Runs `f` on the catalog, saves it, and broadcasts. `structural` also rebuilds the tray
    /// menu and re-registers hotkeys (instances or settings changed).
    pub fn mutate<T>(&self, structural: bool, f: impl FnOnce(&mut StoreFile) -> Result<T, String>) -> Result<T, String> {
        let out = {
            let mut store = self.store.lock();
            let out = f(&mut store.data)?;
            store.data.settings.sanitize();
            store.save().map_err(|e| e.to_string())?;
            out
        };
        self.changed(structural);
        self.wake_monitor();
        Ok(out)
    }

    /// Saves a little later, coalescing bursts (e.g. dragging the floating launcher).
    pub fn save_soon(self: &Arc<Self>) {
        let gen = self.save_gen.fetch_add(1, Ordering::SeqCst) + 1;
        let me = self.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(700));
            if me.save_gen.load(Ordering::SeqCst) == gen {
                let _ = me.store.lock().save();
            }
        });
    }

    pub fn changed(&self, structural: bool) {
        self.broadcast();
        if structural {
            crate::tray::refresh(&self.app, self);
        }
    }

    pub fn broadcast(&self) {
        let snap = crate::snapshot::build(self);
        let _ = self.app.emit("state", &snap);
    }

    pub fn wake_monitor(&self) {
        let _ = self.monitor_tx.lock().send(());
    }

    /// A few quick rescans right after a launch, so the new window is recognised (and given its
    /// taskbar identity) within a moment instead of the next regular scan.
    pub fn wake_monitor_burst(self: &Arc<Self>) {
        let me = self.clone();
        std::thread::spawn(move || {
            for ms in [350u64, 900, 1800, 3500, 7000] {
                std::thread::sleep(Duration::from_millis(ms));
                me.wake_monitor();
            }
        });
    }

    pub fn instance(&self, id: &str) -> Option<Instance> {
        self.store.lock().data.instances.iter().find(|i| i.id == id).cloned()
    }

    pub fn run_info(&self, id: &str) -> Option<RunInfo> {
        self.runtime.read().by_instance.get(id).cloned()
    }

    fn plan(&self, id: &str) -> Result<LaunchPlan, String> {
        let store = self.store.lock();
        let data = &store.data;
        let inst = data.instances.iter().find(|i| i.id == id).ok_or("instance-missing")?;
        let install = data.installations.iter().find(|i| i.id == inst.installation_id).ok_or("installation-missing")?;
        if !install.present {
            return Err("installation-missing".into());
        }
        let adapter = self.registry.get(&install.adapter).ok_or("adapter-missing")?;
        let ctx = Ctx { env: &self.env, installations: &data.installations };
        adapter.launch_plan(install, inst, &ctx)
    }

    /// Starts the instance, or brings it forward when it already runs (setting `focusIfRunning`).
    pub fn launch(self: &Arc<Self>, id: &str) -> Result<(), String> {
        let inst = self.instance(id).ok_or("instance-missing")?;
        if self.settings().focus_if_running {
            if let Some(run) = self.run_info(id) {
                if launcher_platform::focus_pids(&run.pids) {
                    log::info!("focus {id} (already running)");
                    return Ok(());
                }
            }
        }
        if let Some(dir) = inst.profile.dir() {
            std::fs::create_dir_all(dir).map_err(|e| format!("profile-dir: {e}"))?;
        }
        let plan = self.plan(id)?;
        let pid = spawn(&plan).map_err(|e| format!("spawn: {e}"))?;
        log::info!("launched {id} pid={pid} program={}", plan.program.display());
        let _ = self.mutate(false, |d| {
            if let Some(i) = d.instances.iter_mut().find(|i| i.id == id) {
                i.last_launched_at = Some(Utc::now());
                i.launch_count += 1;
            }
            Ok(())
        });
        self.wake_monitor_burst();
        if self.settings().hide_after_launch {
            crate::windows::hide_main(&self.app);
        }
        Ok(())
    }

    pub fn focus(&self, id: &str) -> bool {
        self.run_info(id).map(|r| launcher_platform::focus_pids(&r.pids)).unwrap_or(false)
    }

    /// Closes the instance's windows the way its × button would.
    pub fn close_windows(&self, id: &str) -> bool {
        self.run_info(id).map(|r| launcher_platform::close_pids(&r.pids)).unwrap_or(false)
    }

    /// Ends the instance's processes (apps that keep running in their tray stop too).
    pub fn quit(self: &Arc<Self>, id: &str) -> usize {
        let Some(run) = self.run_info(id) else { return 0 };
        let mut scanner = launcher_platform::process::ProcessScanner::new();
        let n = scanner.kill(&run.pids);
        self.wake_monitor_burst();
        n
    }

    pub fn restart(self: &Arc<Self>, id: &str) -> Result<(), String> {
        self.quit(id);
        let me = self.clone();
        let id = id.to_string();
        std::thread::spawn(move || {
            for _ in 0..30 {
                std::thread::sleep(Duration::from_millis(200));
                me.wake_monitor();
                if me.run_info(&id).is_none() {
                    break;
                }
            }
            if let Err(e) = me.launch(&id) {
                log::warn!("restart {id}: {e}");
            }
        });
        Ok(())
    }

    pub fn launch_autostart_instances(self: &Arc<Self>) {
        let ids: Vec<String> = self.store.lock().data.instances.iter().filter(|i| i.launch.autostart).map(|i| i.id.clone()).collect();
        for id in ids {
            if let Err(e) = self.launch(&id) {
                log::warn!("autostart {id}: {e}");
            }
        }
    }

    /// Launches the Claude instance with the most headroom (lowest of its 5-hour and weekly use).
    pub fn launch_best_claude(self: &Arc<Self>) -> Result<String, String> {
        let candidates: Vec<String> = {
            let store = self.store.lock();
            store
                .data
                .instances
                .iter()
                .filter(|i| store.data.installations.iter().any(|x| x.id == i.installation_id && x.adapter == "claude" && x.present))
                .map(|i| i.id.clone())
                .collect()
        };
        let usage = self.usage.lock();
        let score = |id: &String| -> f64 { usage.get(id).and_then(|u| u.headroom_score()).unwrap_or(50.0) };
        let best = candidates.iter().min_by(|a, b| score(a).total_cmp(&score(b))).cloned().ok_or("no-claude")?;
        drop(usage);
        self.launch(&best)?;
        Ok(best)
    }

    pub fn sync_autostart(&self) {
        use tauri_plugin_autostart::ManagerExt;
        let want = self.settings().start_with_os;
        let al = self.app.autolaunch();
        let have = al.is_enabled().unwrap_or(false);
        let r = match (want, have) {
            (true, false) => al.enable(),
            (false, true) => al.disable(),
            _ => Ok(()),
        };
        if let Err(e) = r {
            log::warn!("autostart: {e}");
        }
    }

    /// Folder that really holds the instance's data (Store packages may keep it virtualized).
    pub fn profile_dir_of(&self, inst: &Instance) -> Option<PathBuf> {
        let store = self.store.lock();
        catalog::physical_profile_dir(&store.data, &self.registry, &self.env, inst)
    }
}

/// Starts the program detached from the launcher (closing the launcher leaves it running).
fn spawn(plan: &LaunchPlan) -> std::io::Result<u32> {
    let mut c = Command::new(&plan.program);
    c.args(&plan.args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    for (k, v) in &plan.env {
        c.env(k, v);
    }
    if let Some(cwd) = &plan.cwd {
        if cwd.is_dir() {
            c.current_dir(cwd);
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        c.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }
    Ok(c.spawn()?.id())
}
