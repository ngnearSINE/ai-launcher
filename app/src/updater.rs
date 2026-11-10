//! Automatic updates from GitHub Releases (`latest.json` written by the release workflow).
//!
//! Same policy as the team's other desktop tools: check 30 s after start and every 6 hours,
//! download in the background, then install either when the user clicks "Restart to update" or
//! during the nightly window (default 03:00–04:00) when automatic install is on. Updates are
//! signed; the public key lives in `tauri.conf.json`. Only installed builds update themselves:
//! the Windows installer build and the Linux AppImage. Portable copies, `.deb` (apt) and
//! development builds report `unsupported`.
//!
//! Instances started by the launcher are separate processes, so restarting the launcher to
//! install never closes them.

use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Local, Timelike, Utc};
use serde::Serialize;
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::core::Core;

const FIRST_CHECK: Duration = Duration::from_secs(30);
const CHECK_EVERY: Duration = Duration::from_secs(6 * 3600);
const TICK: Duration = Duration::from_secs(10 * 60);

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum UpdateState {
    #[serde(rename_all = "camelCase")]
    Unsupported { reason: String },
    Idle,
    Checking,
    #[serde(rename_all = "camelCase")]
    Latest { checked_at: DateTime<Utc> },
    #[serde(rename_all = "camelCase")]
    Downloading { version: String, percent: u8, notes: Option<String> },
    #[serde(rename_all = "camelCase")]
    Ready { version: String, notes: Option<String> },
    Installing,
    #[serde(rename_all = "camelCase")]
    Error { message: String, checked_at: DateTime<Utc> },
}

/// Why this build cannot update itself, if it cannot.
pub fn unsupported_reason() -> Option<&'static str> {
    if cfg!(debug_assertions) {
        return Some("dev");
    }
    #[cfg(windows)]
    {
        let dir = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf()));
        let installed = dir.map(|d| d.join("uninstall.exe").is_file()).unwrap_or(false);
        if !installed {
            return Some("portable");
        }
        None
    }
    #[cfg(target_os = "linux")]
    {
        if std::env::var_os("APPIMAGE").is_none() {
            return Some("package");
        }
        None
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        Some("os")
    }
}

/// Downloaded update waiting for "install".
pub struct Pending {
    update: Update,
    bytes: Vec<u8>,
}

fn set(core: &Core, s: UpdateState) {
    *core.update.lock() = s;
    core.broadcast();
    crate::tray::refresh(&core.app, core);
}

fn in_window(now: DateTime<Local>, hour: u8) -> bool {
    now.hour() == hour as u32
}

pub fn start(core: Arc<Core>) {
    if let Some(reason) = unsupported_reason() {
        *core.update.lock() = UpdateState::Unsupported { reason: reason.into() };
        return;
    }
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(FIRST_CHECK).await;
        let mut last_check = std::time::Instant::now() - CHECK_EVERY;
        loop {
            let s = core.settings();
            if s.check_updates && last_check.elapsed() >= CHECK_EVERY {
                last_check = std::time::Instant::now();
                check(&core, false).await;
            }
            // Nightly install of a downloaded update.
            let ready = matches!(*core.update.lock(), UpdateState::Ready { .. });
            if ready && s.auto_install_updates && in_window(Local::now(), s.update_hour) {
                install(&core).await;
            }
            tokio::time::sleep(TICK).await;
        }
    });
}

/// Checks and, when there is a newer version, downloads it. `manual` reports failures as errors
/// the user sees; background checks stay quiet unless something is ready.
pub async fn check(core: &Arc<Core>, manual: bool) {
    let busy = matches!(
        *core.update.lock(),
        UpdateState::Unsupported { .. } | UpdateState::Checking | UpdateState::Downloading { .. } | UpdateState::Ready { .. } | UpdateState::Installing
    );
    if busy {
        return;
    }
    set(core, UpdateState::Checking);
    let found = match core.app.updater() {
        Ok(u) => u.check().await,
        Err(e) => Err(e),
    };
    let update = match found {
        Ok(Some(u)) => u,
        Ok(None) => {
            // Logged so the log shows background checks are running, not only when they fail.
            log::info!("update check: up to date ({})", core.app.package_info().version);
            return set(core, UpdateState::Latest { checked_at: Utc::now() });
        }
        Err(e) => {
            let text = e.to_string();
            log::warn!("update check: {text}");
            // No release published yet looks like a 404 on latest.json; that is "up to date".
            if text.contains("404") || text.contains("valid release JSON") {
                log::info!("update check: no release published, treated as up to date");
                return set(core, UpdateState::Latest { checked_at: Utc::now() });
            }
            let message = if text.contains("error sending request") || text.contains("dns") { "network" } else { "check" };
            let _ = manual;
            return set(core, UpdateState::Error { message: message.into(), checked_at: Utc::now() });
        }
    };
    let version = update.version.clone();
    let notes = update.body.clone().filter(|b| !b.trim().is_empty());
    log::info!("update {version} available, downloading");
    set(core, UpdateState::Downloading { version: version.clone(), percent: 0, notes: notes.clone() });

    let mut got: u64 = 0;
    let mut last_pct = 0u8;
    let me = core.clone();
    let (v2, n2) = (version.clone(), notes.clone());
    let result = update
        .download(
            move |chunk, total| {
                got += chunk as u64;
                if let Some(t) = total.filter(|t| *t > 0) {
                    let pct = ((got * 100) / t).min(100) as u8;
                    if pct >= last_pct + 5 {
                        last_pct = pct;
                        set(&me, UpdateState::Downloading { version: v2.clone(), percent: pct, notes: n2.clone() });
                    }
                }
            },
            || {},
        )
        .await;
    match result {
        Ok(bytes) => {
            *core.update_pending.lock().await = Some(Pending { update, bytes });
            set(core, UpdateState::Ready { version: version.clone(), notes });
            let s = core.settings();
            if s.auto_install_updates && in_window(Local::now(), s.update_hour) {
                install(core).await;
            } else {
                notify_ready(core, &version);
            }
        }
        Err(e) => {
            log::warn!("update download: {e}");
            set(core, UpdateState::Error { message: "download".into(), checked_at: Utc::now() });
        }
    }
}

fn notify_ready(core: &Core, version: &str) {
    use tauri_plugin_notification::NotificationExt;
    let lang = crate::i18n::lang(&core.settings());
    let _ = core
        .app
        .notification()
        .builder()
        .title(crate::i18n::t(lang, "update.ready.title"))
        .body(crate::i18n::t(lang, "update.ready.body").replace("{v}", version))
        .show();
}

/// Installs the downloaded update. The Windows installer runs passively and the launcher exits
/// for it; on Linux the AppImage is replaced and the launcher restarts itself.
pub async fn install(core: &Arc<Core>) -> bool {
    let Some(p) = core.update_pending.lock().await.take() else { return false };
    set(core, UpdateState::Installing);
    // Settings and catalog are already on disk; make sure the latest state is flushed.
    let _ = core.store.lock().save();
    match p.update.install(p.bytes) {
        Ok(()) => {
            core.app.restart();
        }
        Err(e) => {
            log::warn!("update install: {e}");
            set(core, UpdateState::Error { message: "install".into(), checked_at: Utc::now() });
            false
        }
    }
}
