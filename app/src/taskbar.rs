//! Gives each instance's windows their own taskbar button (Windows).
//!
//! Every app window that belongs to a running instance gets an AppUserModelID of
//! `AiLauncher.<instance id>` when "separate taskbar button" is in effect for that instance, plus
//! relaunch properties so a pinned button starts that exact instance through the launcher.
//!
//! Measured on Windows 11 (build 26300):
//! - Setting the ID on another process's window works; reading it back from outside does not, so
//!   the launcher remembers which windows it tagged.
//! - Writing an empty value does not return a window to its app's group. Packaged (Store) apps are
//!   merged back by writing their own package AppUserModelID; other apps merge on their next start.
//! - With relaunch properties present the button icon comes from RelaunchIconResource. A file
//!   under AppData shows as a blank icon (the same file under the user folder works), so icons
//!   live in `~/.ai-launcher/icons`. Paths are also passed in 8.3 form to be safe with spaces.
//! - The taskbar keeps the first icon it saw for an ID; colour changes show after the app restarts.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use launcher_core::catalog::effective;
use launcher_platform::badge;
use launcher_platform::win::{process_app_id, short_path, taskbar as tb, watcher, windows as w};
use parking_lot::Mutex;

use crate::core::Core;

pub const APP_ID_PREFIX: &str = "AiLauncher.";

/// hwnd → signature of what the launcher last wrote ("-" = left alone / restored).
static APPLIED: Mutex<Option<HashMap<isize, String>>> = Mutex::new(None);
/// Instances whose windows could not be merged back until the app restarts.
static NEEDS_RESTART: Mutex<Option<HashSet<String>>> = Mutex::new(None);

pub fn needs_restart() -> HashSet<String> {
    NEEDS_RESTART.lock().clone().unwrap_or_default()
}

fn icon_resource(p: &Path) -> String {
    format!("{},0", short_path(p).display())
}

pub fn app_id(instance_id: &str) -> String {
    format!("{APP_ID_PREFIX}{instance_id}")
}

pub fn relaunch_command(instance_id: &str) -> String {
    let exe = std::env::current_exe().unwrap_or_default();
    format!("\"{}\" launch {}", exe.display(), instance_id)
}

pub fn start_watcher(core: Arc<Core>) {
    watcher::start(move |hwnd, pid| {
        let known = core.runtime.read().pid_index.contains_key(&pid);
        if known {
            launcher_platform::win::com_init();
            apply_one(&core, hwnd, pid);
        }
    });
}

/// Badge icon (`.ico`) for an instance: the app's icon with the instance colour, cached by colour.
pub fn badge_icon(core: &Core, instance_id: &str, color: &str, exe: Option<&Path>) -> Option<PathBuf> {
    let path = core.icons_dir().join(format!("{instance_id}-{}.ico", color.trim_start_matches('#').to_lowercase()));
    if path.is_file() {
        return Some(path);
    }
    let rgb = badge::parse_hex(color);
    let base = exe
        .and_then(|e| launcher_platform::win::icon::extract_rgba(e, 0, 256))
        .unwrap_or_else(|| badge::plain_tile(256, rgb));
    badge::write_ico(&base, rgb, &path).ok().map(|_| path)
}

/// Desired identity for the windows of `instance_id`, or `None` to leave them in the app's group.
fn desired(core: &Core, instance_id: &str) -> Option<tb::WindowIdentity> {
    let (inst, settings) = {
        let store = core.store.lock();
        let inst = store.data.instances.iter().find(|i| i.id == instance_id)?.clone();
        (inst, store.data.settings.clone())
    };
    let eff = effective(&settings, &inst);
    if !eff.taskbar_separate {
        return None;
    }
    let exe = core.run_info(instance_id).and_then(|r| r.exe);
    let icon = if eff.taskbar_badge {
        badge_icon(core, &inst.id, &inst.color, exe.as_deref()).map(|p| icon_resource(&p))
    } else {
        exe.as_deref().map(icon_resource)
    };
    Some(tb::WindowIdentity {
        app_id: app_id(&inst.id),
        relaunch_command: eff.relaunch_through_launcher.then(|| relaunch_command(&inst.id)),
        display_name: inst.alias.clone(),
        icon,
    })
}

fn signature(id: &Option<tb::WindowIdentity>) -> String {
    match id {
        Some(i) => format!("{}|{:?}|{}|{:?}", i.app_id, i.relaunch_command, i.display_name, i.icon),
        None => "-".into(),
    }
}

fn apply_window(hwnd: isize, pid: u32, instance_id: &str, want: &Option<tb::WindowIdentity>, applied: &mut HashMap<isize, String>) {
    let sig = signature(want);
    let before = applied.get(&hwnd).cloned();
    if before.as_deref() == Some(sig.as_str()) {
        return;
    }
    let result = match want {
        Some(id) => {
            if let Some(set) = NEEDS_RESTART.lock().as_mut() {
                set.remove(instance_id);
            }
            tb::apply(hwnd, id)
        }
        // Never tagged by the launcher: leave the app's own grouping alone.
        None if before.as_deref().map(|b| b == "-").unwrap_or(true) => Ok(()),
        None => match process_app_id(pid) {
            Some(own) => tb::apply(hwnd, &tb::WindowIdentity { app_id: own, relaunch_command: None, display_name: String::new(), icon: None }),
            None => {
                NEEDS_RESTART.lock().get_or_insert_with(HashSet::new).insert(instance_id.to_string());
                Ok(())
            }
        },
    };
    match result {
        Ok(()) => {
            applied.insert(hwnd, sig);
        }
        Err(e) => log::debug!("taskbar identity for {hwnd:#x}: {e}"),
    }
}

fn apply_one(core: &Core, hwnd: isize, pid: u32) {
    if !w::is_app_window(hwnd) {
        return;
    }
    let Some(instance_id) = core.runtime.read().pid_index.get(&pid).cloned() else { return };
    let want = desired(core, &instance_id);
    let mut guard = APPLIED.lock();
    apply_window(hwnd, pid, &instance_id, &want, guard.get_or_insert_with(HashMap::new));
}

/// Brings every instance window in line with the current settings.
pub fn sync(core: &Core) {
    let index = core.runtime.read().pid_index.clone();
    let mut guard = APPLIED.lock();
    let applied = guard.get_or_insert_with(HashMap::new);
    if index.is_empty() && applied.is_empty() {
        return;
    }
    let windows = w::app_windows();
    let mut wants: HashMap<String, Option<tb::WindowIdentity>> = HashMap::new();
    for (hwnd, pid) in &windows {
        let Some(instance_id) = index.get(pid) else { continue };
        let want = wants.entry(instance_id.clone()).or_insert_with(|| desired(core, instance_id)).clone();
        apply_window(*hwnd, *pid, instance_id, &want, applied);
    }
    applied.retain(|h, _| windows.iter().any(|(x, _)| x == h));
    // An instance that is no longer running has merged back on its own.
    if let Some(set) = NEEDS_RESTART.lock().as_mut() {
        set.retain(|id| index.values().any(|v| v == id));
    }
}

/// Re-apply on the next sync (after names, colours or settings change). Windows the launcher
/// tagged keep a non-"-" marker so turning separation off can still restore them.
pub fn invalidate() {
    if let Some(m) = APPLIED.lock().as_mut() {
        for v in m.values_mut() {
            if v != "-" {
                *v = "?".into();
            }
        }
    }
}
