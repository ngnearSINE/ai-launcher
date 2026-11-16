//! Usage polling. Each instance is read from its own profile folder only (never another
//! profile, the CLI or the app's default folder unless that *is* the instance's folder), or
//! from the one command-line sign-in the user linked to it. Instances that read the same
//! source share one request. Requests are spaced per source, and HTTP 429 backs off
//! progressively.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use launcher_core::catalog::effective;
use launcher_core::{Instance, UsageKind};
use launcher_usage::{agy, claude, codex, cursor, grok_cli, QuotaKind, UsageError, UsageReport};
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

    /// Higher = less headroom. Uses the worst of the 5-hour, weekly and monthly (Cursor plan)
    /// windows of the first organisation the instance shows (`shown`, from [`shown_orgs`]).
    pub fn headroom_score(&self, shown: &[String]) -> Option<f64> {
        let report = self.report.as_ref()?;
        let org = report.orgs.iter().find(|o| shown.contains(&o.org_uuid)).or(report.orgs.first())?;
        org.quotas
            .iter()
            .filter(|q| matches!(q.kind, QuotaKind::FiveHour | QuotaKind::Weekly | QuotaKind::Monthly))
            .map(|q| q.used_percent)
            .reduce(f64::max)
    }
}

/// Organisations instance `id` shows (its own pick, else the ones no other instance of the
/// same account already stands for). Empty when there is no report yet.
pub fn shown_orgs(instances: &[Instance], usage: &HashMap<String, UsageEntry>, id: &str) -> Vec<String> {
    let Some(me) = usage.get(id).and_then(|e| e.report.as_ref()) else { return Vec::new() };
    let pick_of = |iid: &str| instances.iter().find(|i| i.id == iid).map(|i| i.advanced.usage_orgs.as_slice()).unwrap_or(&[]);
    let others: Vec<(&UsageReport, &[String])> = usage
        .iter()
        .filter(|(k, _)| k.as_str() != id && instances.iter().any(|i| &i.id == *k))
        .filter_map(|(k, e)| e.report.as_ref().map(|r| (r, pick_of(k))))
        .collect();
    launcher_usage::shown_orgs(me, pick_of(id), &others)
}

/// Headroom score of each listed instance, using what each one shows.
pub fn scores(core: &Core, ids: &[String]) -> HashMap<String, f64> {
    let instances = core.store.lock().data.instances.clone();
    let usage = core.usage.lock();
    ids.iter()
        .filter_map(|id| {
            let shown = shown_orgs(&instances, &usage, id);
            usage.get(id).and_then(|u| u.headroom_score(&shown)).map(|s| (id.clone(), s))
        })
        .collect()
}

// ---- last good numbers on disk -----------------------------------------------------------------
// Every instance's last report is kept in `usage-cache.json` (numbers only: no token, no
// address), so after a restart — an update, say — the floating bar and the cards show them at
// once, marked by their age, instead of nothing while the service is still rate limiting.

#[derive(serde::Serialize, serde::Deserialize)]
struct Cached {
    source: String,
    report: UsageReport,
}

fn cache_path(core: &Core) -> std::path::PathBuf {
    core.config_dir.join("usage-cache.json")
}

static CACHE: std::sync::OnceLock<parking_lot::Mutex<HashMap<String, Cached>>> = std::sync::OnceLock::new();

fn cache(core: &Core) -> &'static parking_lot::Mutex<HashMap<String, Cached>> {
    CACHE.get_or_init(|| {
        let read = std::fs::read(cache_path(core)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        parking_lot::Mutex::new(read)
    })
}

/// A fresh entry, with the last numbers this folder gave if there are any.
fn entry_for(core: &Core, id: &str, source: String) -> UsageEntry {
    let mut e = UsageEntry::new(source);
    if let Some(c) = cache(core).lock().get(id).filter(|c| c.source == e.source) {
        e.report = Some(c.report.clone());
        e.status = "ok";
    }
    e
}

fn remember(core: &Core, id: &str, source: &str, report: &UsageReport) {
    let mut c = cache(core).lock();
    c.insert(id.to_string(), Cached { source: source.to_string(), report: report.clone() });
    if let Ok(bytes) = serde_json::to_vec(&*c) {
        let path = cache_path(core);
        let tmp = path.with_extension("json.tmp");
        if std::fs::write(&tmp, bytes).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
    }
}

fn forget_missing(core: &Core, keep: &[Job]) {
    cache(core).lock().retain(|id, _| keep.iter().any(|j| &j.id == id));
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

/// One instance whose usage is read, and where from.
struct Job {
    id: String,
    kind: UsageKind,
    /// The instance's own profile folder; None for a linked command-line sign-in.
    dir: Option<std::path::PathBuf>,
}

impl Job {
    /// What the numbers come from: the folder, or `cli:<name>`. Instances with the same
    /// source share one request.
    fn source(&self) -> String {
        match (&self.dir, self.kind.cli_id()) {
            (_, Some(cli)) => format!("cli:{cli}"),
            (Some(dir), None) => dir.display().to_string(),
            (None, None) => String::new(),
        }
    }
}

/// Instances that have usage (their app's own, or a CLI the user linked) and where to read it.
fn jobs(core: &Core) -> Vec<Job> {
    let store = core.store.lock();
    let data = &store.data;
    data.instances
        .iter()
        .filter_map(|inst| {
            let kind = launcher_core::catalog::usage_kind(data, &core.registry, inst)?;
            if !effective(&data.settings, inst).usage_enabled {
                return None;
            }
            let dir = if kind.is_cli() {
                None
            } else {
                // Exactly this instance's folder, resolved to where the app really wrote it.
                Some(launcher_core::catalog::physical_profile_dir(data, &core.registry, &core.env, inst)?)
            };
            Some(Job { id: inst.id.clone(), kind, dir })
        })
        .collect()
}

/// Whether the source holds a sign-in of that kind (local files only, no request).
fn has_session(kind: UsageKind, dir: Option<&std::path::Path>) -> bool {
    match (kind, dir) {
        (UsageKind::ClaudeDesktop, Some(d)) => claude::has_session(d),
        (UsageKind::Cursor, Some(d)) => cursor::has_session(cursor::App::Cursor, d),
        (UsageKind::GrokBot, Some(d)) => cursor::has_session(cursor::App::GrokBot, d),
        (UsageKind::CodexCli, _) => codex::account().is_some(),
        (UsageKind::GrokCli, _) => grok_cli::account().is_some(),
        (UsageKind::Antigravity, _) => agy::account().is_some(),
        _ => false,
    }
}

async fn fetch(core: &Core, kind: UsageKind, dir: Option<&std::path::Path>) -> Result<UsageReport, UsageError> {
    match (kind, dir) {
        (UsageKind::ClaudeDesktop, Some(d)) => claude::fetch(&core.http, d).await,
        (UsageKind::Cursor, Some(d)) => cursor::fetch(&core.http, cursor::App::Cursor, d).await,
        (UsageKind::GrokBot, Some(d)) => cursor::fetch(&core.http, cursor::App::GrokBot, d).await,
        (UsageKind::CodexCli, _) => codex::fetch().await,
        (UsageKind::GrokCli, _) => grok_cli::fetch(&core.http).await,
        (UsageKind::Antigravity, _) => agy::fetch().await,
        _ => Err(UsageError::NotSignedIn),
    }
}

async fn tick(core: &Arc<Core>) {
    let jobs = jobs(core);
    let refresh = core.settings().usage_refresh_minutes as i64 * 60;
    {
        let mut usage = core.usage.lock();
        usage.retain(|id, _| jobs.iter().any(|j| &j.id == id));
    }
    forget_missing(core, &jobs);
    // Instances that read the same source, in list order.
    let mut groups: Vec<(String, Vec<&Job>)> = Vec::new();
    for job in &jobs {
        let source = job.source();
        match groups.iter_mut().find(|(s, _)| *s == source) {
            Some((_, list)) => list.push(job),
            None => groups.push((source, vec![job])),
        }
    }
    for (source, group) in groups {
        let first = group[0];
        let ids: Vec<String> = group.iter().map(|j| j.id.clone()).collect();
        let due = {
            let mut usage = core.usage.lock();
            let mut due = false;
            for id in &ids {
                let e = usage.entry(id.clone()).or_insert_with(|| entry_for(core, id, source.clone()));
                if e.source != source {
                    *e = entry_for(core, id, source.clone());
                }
                due |= Utc::now() >= e.next_at;
            }
            due
        };
        if !due {
            continue;
        }
        if !has_session(first.kind, first.dir.as_deref()) {
            let mut usage = core.usage.lock();
            for id in &ids {
                if let Some(e) = usage.get_mut(id) {
                    e.status = "error";
                    e.report = None;
                    e.error = Some(UsageError::NotSignedIn);
                    e.next_at = Utc::now() + chrono::Duration::seconds(60);
                }
            }
            drop(usage);
            core.broadcast();
            continue;
        }
        for id in &ids {
            set_status(core, id, "loading");
        }
        let result = fetch(core, first.kind, first.dir.as_deref()).await;
        if let Err(err) = &result {
            // Kept short: the reason only (never a token or an address).
            log::info!("usage {}: {err:?}", ids.join(","));
        }
        {
            let mut usage = core.usage.lock();
            let now = Utc::now();
            for id in &ids {
                let Some(e) = usage.get_mut(id) else { continue };
                match &result {
                    Ok(r) => {
                        remember(core, id, &e.source, r);
                        e.status = "ok";
                        e.report = Some(r.clone());
                        e.error = None;
                        e.strikes = 0;
                        e.next_at = now + chrono::Duration::seconds(refresh.max(MIN_INTERVAL_SECS));
                    }
                    Err(err) => {
                        let wait = match err {
                            UsageError::RateLimited(s) => {
                                e.strikes += 1;
                                let backoff = (MIN_INTERVAL_SECS * 2i64.pow(e.strikes.min(5))).min(MAX_BACKOFF_SECS);
                                (*s as i64).max(backoff)
                            }
                            UsageError::Network(_) => 60,
                            _ => refresh.max(MIN_INTERVAL_SECS),
                        };
                        // Keep the last good numbers visible; the card marks them as stale.
                        e.status = if e.report.is_some() { "ok" } else { "error" };
                        e.error = Some(err.clone());
                        e.next_at = now + chrono::Duration::seconds(wait);
                    }
                }
            }
        }
        for id in &ids {
            alerts(core, id);
        }
        core.broadcast();
    }
}

/// Last seen state per (instance/organisation, window): over the critical line or not, and
/// which reset it belongs to. Keyed by organisation too, so a change of the shown organisation
/// is not mistaken for a reset. Only changes seen while the launcher runs raise a notification, so restarting
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
    let instances = core.store.lock().data.instances.clone();
    let (report, shown) = {
        let usage = core.usage.lock();
        (usage.get(id).and_then(|e| e.report.clone()), shown_orgs(&instances, &usage, id))
    };
    let Some(report) = report else { return };
    let Some(org) = report.orgs.iter().find(|o| shown.contains(&o.org_uuid)).or(report.orgs.first()) else { return };
    let seen_key = format!("{id}/{}", org.org_uuid);
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
        let prev = SEEN.lock().insert((seen_key.clone(), kind), (crit, q.resets_at));
        let Some((was_crit, was_reset)) = prev else { continue };
        let window = crate::i18n::t(lang, wkey);
        if crit && (!was_crit || !same_reset(was_reset, q.resets_at)) && s.notify_low {
            let left = (100.0 - q.used_percent).max(0.0).round();
            let reset = q.resets_at.map(|r| until_text(lang, r)).unwrap_or_default();
            let mut body = tr("notify.low.body", &[("left", left.to_string()), ("reset", reset)]);
            if let Some((name, room)) = roomiest_other(core, id) {
                body = format!("{body}\n{}", tr("notify.low.suggest", &[("name", name), ("left", room.to_string())]));
            }
            notify(core, &tr("notify.low.title", &[("name", inst.alias.clone()), ("window", window)]), &body);
        } else if !crit && was_crit && !same_reset(was_reset, q.resets_at) && s.notify_reset {
            notify(core, &tr("notify.reset.title", &[("name", inst.alias.clone()), ("window", window)]), &crate::i18n::t(lang, "notify.reset.body"));
        }
    }
}

/// Whether two reset times belong to the same window. The service reports the same reset with
/// a slightly different fraction of a second on every read, so exact comparison made a "running
/// low" notice repeat on every refresh (seen 2026-11-12); windows are hours apart.
fn same_reset(a: Option<DateTime<Utc>>, b: Option<DateTime<Utc>>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => (a - b).num_minutes().abs() < 10,
        (None, None) => true,
        _ => false,
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
    let ids: Vec<String> = claude.iter().map(|(i, _)| i.clone()).collect();
    let scores = scores(core, &ids);
    claude
        .into_iter()
        .filter_map(|(iid, alias)| scores.get(&iid).map(|used| (alias, *used)))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jitter_in_reset_time_is_the_same_window() {
        let t = |s: &str| Some(DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc));
        assert!(same_reset(t("2026-11-12T03:00:00.086636Z"), t("2026-11-12T03:00:00.323816Z")));
        assert!(same_reset(t("2026-11-12T05:19:59.771224Z"), t("2026-11-12T05:20:00.006777Z")));
        assert!(!same_reset(t("2026-11-12T03:00:00Z"), t("2026-11-12T08:00:00Z")));
        assert!(!same_reset(t("2026-11-12T03:00:00Z"), None));
    }
}
