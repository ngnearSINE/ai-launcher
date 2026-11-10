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
            // Exactly this instance's folder, resolved to where the app really wrote it.
            let dir = launcher_core::catalog::physical_profile_dir(data, &core.registry, &core.env, inst)?;
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
        alerts(core, &id);
        core.broadcast();
    }
}

/// Last seen state per (instance, window): over the critical line or not, and which reset it
/// belongs to. Only changes seen while the launcher runs raise a notification, so restarting
/// it never repeats one.
static SEEN: std::sync::LazyLock<parking_lot::Mutex<std::collections::HashMap<(String, &'static str), (bool, Option<DateTime<Utc>>)>>> =
    std::sync::LazyLock::new(Default::default);

/// "Running low" (with the Claude account that has the most room) and "reset" notifications.
fn alerts(core: &Core, id: &str) {
    let s = core.settings();
    if !s.notify_low && !s.notify_reset {
        return;
    }
    let Some(inst) = core.instance(id) else { return };
    let Some(report) = core.usage.lock().get(id).and_then(|e| e.report.clone()) else { return };
    let pick = &inst.advanced.usage_orgs;
    let Some(org) = report.orgs.iter().find(|o| pick.is_empty() || pick.contains(&o.org_uuid)) else { return };
    let lang = crate::i18n::lang(&s);
    let tr = |key: &str, vars: &[(&str, String)]| {
        let mut out = crate::i18n::t(lang, key);
        for (k, v) in vars {
            out = out.replace(&format!("{{{k}}}"), v);
        }
        out
    };
    for q in &org.quotas {
        let (kind, wkey) = match q.kind {
            QuotaKind::FiveHour => ("5h", "window.5h"),
            QuotaKind::Weekly => ("weekly", "window.weekly"),
            _ => continue,
        };
        let crit = q.used_percent >= s.usage_crit_percent as f64;
        let prev = SEEN.lock().insert((id.to_string(), kind), (crit, q.resets_at));
        let Some((was_crit, was_reset)) = prev else { continue };
        let window = crate::i18n::t(lang, wkey);
        if crit && (!was_crit || was_reset != q.resets_at) && s.notify_low {
            let left = (100.0 - q.used_percent).max(0.0).round();
            let reset = q.resets_at.map(|r| until_text(lang, r)).unwrap_or_default();
            let mut body = tr("notify.low.body", &[("left", left.to_string()), ("reset", reset)]);
            if let Some((name, room)) = roomiest_other(core, id) {
                body = format!("{body}\n{}", tr("notify.low.suggest", &[("name", name), ("left", room.to_string())]));
            }
            notify(core, &tr("notify.low.title", &[("name", inst.alias.clone()), ("window", window)]), &body);
        } else if !crit && was_crit && was_reset != q.resets_at && s.notify_reset {
            notify(core, &tr("notify.reset.title", &[("name", inst.alias.clone()), ("window", window)]), &crate::i18n::t(lang, "notify.reset.body"));
        }
    }
}

/// The other Claude instance with the most room, as (alias, % left of its tighter window).
fn roomiest_other(core: &Core, id: &str) -> Option<(String, i64)> {
    let claude: Vec<(String, String)> = {
        let store = core.store.lock();
        let data = &store.data;
        data.instances
            .iter()
            .filter(|i| i.id != id && data.installations.iter().any(|x| x.id == i.installation_id && x.adapter == "claude" && x.present))
            .map(|i| (i.id.clone(), i.alias.clone()))
            .collect()
    };
    let usage = core.usage.lock();
    claude
        .into_iter()
        .filter_map(|(iid, alias)| usage.get(&iid).and_then(|u| u.headroom_score()).map(|used| (alias, used)))
        .filter(|(_, used)| *used < 90.0)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(alias, used)| (alias, (100.0 - used).round() as i64))
}

fn until_text(lang: &str, at: DateTime<Utc>) -> String {
    let mins = (at - Utc::now()).num_minutes().max(1);
    let (d, h, m) = (mins / 1440, (mins % 1440) / 60, mins % 60);
    let ko = lang == "ko";
    match (d, h) {
        (d, h) if d > 0 => if ko { format!("{d}일 {h}시간") } else { format!("{d}d {h}h") },
        (_, h) if h > 0 => if ko { format!("{h}시간 {m}분") } else { format!("{h}h {m}m") },
        _ => if ko { format!("{m}분") } else { format!("{m}m") },
    }
}

fn notify(core: &Core, title: &str, body: &str) {
    use tauri_plugin_notification::NotificationExt;
    log::info!("notify: {title}");
    let _ = core.app.notification().builder().title(title).body(body).show();
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
