//! Usage polling. Each instance is read from its own profile folder only (never another
//! profile, the CLI or the app's default folder unless that *is* the instance's folder).
//! Requests are spaced per instance, and HTTP 429 backs off progressively.

use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use launcher_core::catalog::effective;
use launcher_core::UsageKind;
use launcher_usage::{claude, QuotaKind, UsageError, UsageReport};
use serde::Serialize;

use crate::core::Core;

const MIN_INTERVAL_SECS: i64 = 120;
const MAX_BACKOFF_SECS: i64 = 30 * 60;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageEntry {
    /// `idle` | `loading` | `ok` | `error`.
    pub status: &'static str,
    pub report: Option<UsageReport>,
    pub error: Option<UsageError>,
    pub next_at: DateTime<Utc>,
    #[serde(skip)]
    pub strikes: u32,
    /// Profile folder the numbers come from (shown in the card details).
    pub source: String,
}

impl UsageEntry {
    fn new(source: String) -> Self {
        Self { status: "idle", report: None, error: None, next_at: Utc::now(), strikes: 0, source }
    }

    /// Higher = less headroom. Uses the worst of 5-hour and weekly of the first organisation.
    pub fn headroom_score(&self) -> Option<f64> {
        let org = self.report.as_ref()?.orgs.first()?;
        org.quotas
            .iter()
            .filter(|q| matches!(q.kind, QuotaKind::FiveHour | QuotaKind::Weekly))
            .map(|q| q.used_percent)
            .reduce(f64::max)
    }
}

pub fn start(core: Arc<Core>) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(800)).await;
        loop {
            tick(&core).await;
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(20)) => {}
                _ = core.usage_wake.notified() => {}
            }
        }
    });
}

/// Instances that have usage and the folder each one must be read from.
fn jobs(core: &Core) -> Vec<(String, std::path::PathBuf)> {
    let store = core.store.lock();
    let data = &store.data;
    data.instances
        .iter()
        .filter_map(|inst| {
            let install = data.installations.iter().find(|i| i.id == inst.installation_id)?;
            let adapter = core.registry.get(&install.adapter)?;
            if adapter.usage(install) != Some(UsageKind::ClaudeDesktop) || !effective(&data.settings, inst).usage_enabled {
                return None;
            }
            let dir = match inst.profile.dir() {
                Some(d) => d.clone(),
                None => adapter.default_profile_dir(install, &core.env)?,
            };
            Some((inst.id.clone(), dir))
        })
        .collect()
}

async fn tick(core: &Arc<Core>) {
    let jobs = jobs(core);
    let refresh = core.settings().usage_refresh_minutes as i64 * 60;
    {
        let mut usage = core.usage.lock();
        usage.retain(|id, _| jobs.iter().any(|(j, _)| j == id));
    }
    for (id, dir) in jobs {
        let source = dir.display().to_string();
        let due = {
            let mut usage = core.usage.lock();
            let e = usage.entry(id.clone()).or_insert_with(|| UsageEntry::new(source.clone()));
            if e.source != source {
                *e = UsageEntry::new(source.clone());
            }
            Utc::now() >= e.next_at
        };
        if !due {
            continue;
        }
        if !claude::has_session(&dir) {
            let mut usage = core.usage.lock();
            if let Some(e) = usage.get_mut(&id) {
                e.status = "error";
                e.report = None;
                e.error = Some(UsageError::NotSignedIn);
                e.next_at = Utc::now() + chrono::Duration::seconds(60);
            }
            drop(usage);
            core.broadcast();
            continue;
        }
        set_status(core, &id, "loading");
        let result = claude::fetch(&core.http, &dir).await;
        {
            let mut usage = core.usage.lock();
            let Some(e) = usage.get_mut(&id) else { continue };
            let now = Utc::now();
            match result {
                Ok(r) => {
                    e.status = "ok";
                    e.report = Some(r);
                    e.error = None;
                    e.strikes = 0;
                    e.next_at = now + chrono::Duration::seconds(refresh.max(MIN_INTERVAL_SECS));
                }
                Err(err) => {
                    let wait = match err {
                        UsageError::RateLimited(s) => {
                            e.strikes += 1;
                            let backoff = (MIN_INTERVAL_SECS * 2i64.pow(e.strikes.min(5))).min(MAX_BACKOFF_SECS);
                            (s as i64).max(backoff)
                        }
                        UsageError::Network(_) => 60,
                        _ => refresh.max(MIN_INTERVAL_SECS),
                    };
                    // Keep the last good numbers visible; the card marks them as stale.
                    e.status = if e.report.is_some() { "ok" } else { "error" };
                    e.error = Some(err);
                    e.next_at = now + chrono::Duration::seconds(wait);
                }
            }
        }
        core.broadcast();
    }
}

fn set_status(core: &Core, id: &str, status: &'static str) {
    if let Some(e) = core.usage.lock().get_mut(id) {
        e.status = status;
    }
    core.broadcast();
}

/// Makes one instance (or all) due now and wakes the poller. Respects the minimum spacing.
pub fn refresh_now(core: &Core, id: Option<&str>) {
    {
        let mut usage = core.usage.lock();
        let floor = Utc::now() - chrono::Duration::seconds(1);
        for (k, e) in usage.iter_mut() {
            if id.map(|i| i == k).unwrap_or(true) {
                let earliest = e.report.as_ref().map(|r| r.fetched_at + chrono::Duration::seconds(30)).unwrap_or(floor);
                if !matches!(e.error, Some(UsageError::RateLimited(_))) {
                    e.next_at = earliest.max(floor);
                }
            }
        }
    }
    core.usage_wake.notify_one();
}
