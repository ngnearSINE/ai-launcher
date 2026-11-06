//! Watches which instances are running. One cheap process scan every few seconds (or right
//! away when woken after a launch); only the command lines of candidate executables are read.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use launcher_core::adapter::{Ctx, ProcessMatcher};
use launcher_platform::process::ProcessScanner;

use crate::core::{Core, RunInfo};

pub fn start(core: Arc<Core>) {
    let rx = core.monitor_rx.lock().take().expect("monitor started twice");
    std::thread::Builder::new()
        .name("monitor".into())
        .spawn(move || {
            #[cfg(windows)]
            launcher_platform::win::com_init();
            let mut scanner = ProcessScanner::new();
            loop {
                scan(&core, &mut scanner);
                let secs = core.settings().taskbar_rescan_seconds.max(1) as u64;
                let _ = rx.recv_timeout(Duration::from_secs(secs));
                // Coalesce a burst of wake-ups into one scan.
                while rx.try_recv().is_ok() {}
            }
        })
        .expect("spawn monitor");
}

fn matchers(core: &Core) -> Vec<(String, ProcessMatcher)> {
    let store = core.store.lock();
    let data = &store.data;
    let ctx = Ctx { env: &core.env, installations: &data.installations };
    data.instances
        .iter()
        .filter_map(|inst| {
            let install = data.installations.iter().find(|i| i.id == inst.installation_id)?;
            let adapter = core.registry.get(&install.adapter)?;
            Some((inst.id.clone(), adapter.matcher(install, inst, &ctx)))
        })
        .collect()
}

pub fn scan(core: &Arc<Core>, scanner: &mut ProcessScanner) {
    let matchers = matchers(core);
    let names: HashSet<String> = matchers.iter().flat_map(|(_, m)| m.exe_names.iter().cloned()).collect();
    let procs = if names.is_empty() { Vec::new() } else { scanner.scan(&names) };

    let mut by_instance: HashMap<String, RunInfo> = HashMap::new();
    for (id, m) in &matchers {
        let mains: Vec<_> = procs.iter().filter(|p| m.matches(&p.name, p.exe.as_deref(), &p.args, core.env.os)).collect();
        if mains.is_empty() {
            continue;
        }
        let mut pids: Vec<u32> = Vec::new();
        for p in &mains {
            for pid in ProcessScanner::family(&procs, p.pid) {
                if !pids.contains(&pid) {
                    pids.push(pid);
                }
            }
        }
        by_instance.insert(
            id.clone(),
            RunInfo { main_pids: mains.iter().map(|p| p.pid).collect(), pids, exe: mains[0].exe.clone() },
        );
    }

    let changed = {
        let mut rt = core.runtime.write();
        let changed = rt.by_instance != by_instance;
        if changed {
            rt.pid_index = by_instance.iter().flat_map(|(id, r)| r.pids.iter().map(move |p| (*p, id.clone()))).collect();
            rt.by_instance = by_instance;
        }
        changed
    };
    if changed {
        core.broadcast();
        crate::tray::refresh(&core.app, core);
    }
    #[cfg(windows)]
    {
        let before = crate::taskbar::needs_restart();
        crate::taskbar::sync(core);
        if !changed && crate::taskbar::needs_restart() != before {
            core.broadcast();
        }
    }
}
