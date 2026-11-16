//! Gemini usage through the Antigravity CLI (`agy`).
//!
//! `agy -p /usage --output-format json` prints the account's limits without starting a
//! conversation (no turns, no tokens). Auto-update is switched off for the run so no updater
//! window appears. These are Antigravity's model limits, not the Gemini app's.
//!
//! An `agy` that does not know `/usage` would take it as a prompt and spend quota. Such an
//! answer (no `command` block, or any tokens used) switches this reader off until the launcher
//! restarts, so it never asks twice.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::cli::{self, CliAccount};
use crate::model::*;

const TIMEOUT: Duration = Duration::from_secs(30);
static OFF: AtomicBool = AtomicBool::new(false);

/// The `agy` program: `AGY_EXECUTABLE`, else the newest of `PATH` and the installer's folder.
pub fn find_exe() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("AGY_EXECUTABLE").map(PathBuf::from).filter(|p| p.is_file()) {
        return Some(p);
    }
    let name = cli::exe_name("agy");
    let mut found = cli::on_path("agy");
    if let Some(local) = cli::local_app_data() {
        found.push(local.join("agy").join("bin").join(&name));
    }
    if let Some(home) = cli::home() {
        found.push(home.join(".local").join("bin").join(&name));
        found.push(home.join(".agy").join("bin").join(&name));
    }
    cli::newest(found)
}

/// Whether the CLI is installed. It keeps no account details that can be read locally.
pub fn account() -> Option<CliAccount> {
    find_exe().map(|_| CliAccount::default())
}

fn time(v: &Value) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(v.as_str()?).ok().map(|t| t.with_timezone(&Utc))
}

fn used(bucket: &Value) -> Option<f64> {
    let left = bucket["remaining_fraction"].as_f64().filter(|f| f.is_finite())?;
    Some(((1.0 - left) * 100.0).clamp(0.0, 100.0))
}

/// The output of `/usage` → (quotas, fields not shown).
///
/// Measured 2026-11-16 (agy 1.4.12): `{status: "SUCCESS", num_turns: 0, usage: {total_tokens:
/// 0…}, command: {name: "usage", data: {description, groups: [{name, description, buckets: [{id,
/// name, window: "5h" | "weekly", remaining_fraction: 0..1, reset_time}]}]}}}`. The group
/// holding `gemini-5h`/`gemini-weekly` (the Gemini models) gives the 5-hour and weekly lines;
/// the weekly limit of every other group (e.g. a model with its own allowance) is listed by
/// its group name.
pub fn parse_usage(raw: &Value) -> Result<(Vec<Quota>, Vec<String>), UsageError> {
    let spent = raw["usage"]["total_tokens"].as_f64().unwrap_or(0.0) > 0.0 || raw["num_turns"].as_f64().unwrap_or(0.0) > 0.0;
    if raw["command"]["name"].as_str() != Some("usage") || spent {
        return Err(UsageError::Unsupported);
    }
    let data = &raw["command"]["data"];
    let groups = data["groups"].as_array().filter(|g| !g.is_empty()).ok_or(UsageError::Format("agy usage".into()))?;
    let buckets = |g: &Value| g["buckets"].as_array().cloned().unwrap_or_default();
    let is_main = |g: &&Value| buckets(g).iter().any(|b| b["id"].as_str().is_some_and(|id| matches!(id, "gemini-5h" | "gemini-weekly")));
    let main = groups.iter().find(is_main).unwrap_or(&groups[0]);
    let quota = |kind: QuotaKind, label: String, b: &Value| -> Option<Quota> {
        Some(Quota { kind, label, used_percent: used(b)?, resets_at: time(&b["reset_time"]), used_amount: None, limit_amount: None, currency: None })
    };
    let mut quotas = Vec::new();
    for (window, kind, label) in [("5h", QuotaKind::FiveHour, "5h"), ("weekly", QuotaKind::Weekly, "week")] {
        // Several buckets of one window: the one with the least left decides.
        let tightest = buckets(main).into_iter().filter(|b| b["window"] == window).filter_map(|b| quota(kind.clone(), label.into(), &b)).reduce(|a, b| if b.used_percent > a.used_percent { b } else { a });
        quotas.extend(tightest);
    }
    for g in groups.iter().filter(|g| !std::ptr::eq(*g, main)) {
        let name = g["name"].as_str().unwrap_or("Antigravity").chars().take(40).collect::<String>();
        quotas.extend(buckets(g).iter().filter(|b| b["window"] == "weekly").filter_map(|b| quota(QuotaKind::Scoped, name.clone(), b)));
    }
    if quotas.is_empty() {
        return Err(UsageError::Format("agy usage".into()));
    }
    let unknown = data.as_object().map(|o| o.keys().filter(|k| !["groups", "description"].contains(&k.as_str())).cloned().collect()).unwrap_or_default();
    Ok((quotas, unknown))
}

/// Usage of the account the Antigravity CLI is signed in to.
pub async fn fetch() -> Result<UsageReport, UsageError> {
    if OFF.load(Ordering::Relaxed) {
        return Err(UsageError::Unsupported);
    }
    let exe = find_exe().ok_or(UsageError::NotSignedIn)?;
    let out = cli::command(&exe)
        .args(["-p", "/usage", "--output-format", "json"])
        .env("AGY_CLI_DISABLE_AUTO_UPDATE", "true")
        .stdout(std::process::Stdio::piped())
        .output();
    let out = tokio::time::timeout(TIMEOUT, out)
        .await
        .map_err(|_| UsageError::Network("agy timed out".into()))?
        .map_err(|_| UsageError::Format("agy not runnable".into()))?;
    let raw = cli::json_in(&out.stdout).ok_or(UsageError::Format("agy output".into()))?;
    if raw["status"].as_str().is_some_and(|s| s != "SUCCESS") && raw["command"].is_null() {
        // Typically signed out; the CLI says so in words, which are not parsed.
        return Err(UsageError::NotSignedIn);
    }
    let (quotas, unknown_fields) = match parse_usage(&raw) {
        Err(UsageError::Unsupported) => {
            OFF.store(true, Ordering::Relaxed);
            log::warn!("usage agy: /usage not understood, reader off until restart");
            return Err(UsageError::Unsupported);
        }
        other => other?,
    };
    let org = OrgUsage {
        org_uuid: "agy".into(),
        org_name: String::new(),
        org_type: "antigravity".into(),
        plan: String::new(),
        quotas,
        breakdown: Vec::new(),
        unknown_fields,
    };
    Ok(UsageReport { account_uuid: "agy".into(), email: String::new(), orgs: vec![org], fetched_at: Utc::now() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample() -> Value {
        json!({"status": "SUCCESS", "num_turns": 0, "usage": {"total_tokens": 0},
            "command": {"name": "usage", "data": {"description": "…", "groups": [
                {"name": "Gemini Models", "buckets": [
                    {"id": "gemini-weekly", "window": "weekly", "remaining_fraction": 0.9686779379844666, "reset_time": "2026-11-21T02:51:53Z"},
                    {"id": "gemini-5h", "window": "5h", "remaining_fraction": 1, "reset_time": "2026-11-16T16:47:43Z"}]},
                {"name": "Claude and GPT models", "buckets": [
                    {"id": "3p-weekly", "window": "weekly", "remaining_fraction": 1, "reset_time": "2026-11-23T11:47:43Z"},
                    {"id": "3p-5h", "window": "5h", "remaining_fraction": 1}]},
                {"name": "Gemini 4 Argon", "buckets": [
                    {"id": "gemini-argon-wtu-weekly", "window": "weekly", "remaining_fraction": 0.9893239736557007, "reset_time": "2026-11-23T11:46:13Z"}]}]}}})
    }

    #[test]
    fn gemini_group_gives_the_main_windows() {
        let (q, unknown) = parse_usage(&sample()).unwrap();
        assert_eq!(q[0].kind, QuotaKind::FiveHour);
        assert_eq!(q[0].used_percent, 0.0);
        assert_eq!(q[1].kind, QuotaKind::Weekly);
        assert!((q[1].used_percent - 3.13220620155334).abs() < 1e-9);
        let scoped: Vec<_> = q[2..].iter().map(|q| (q.kind.clone(), q.label.as_str())).collect();
        assert_eq!(scoped, [(QuotaKind::Scoped, "Claude and GPT models"), (QuotaKind::Scoped, "Gemini 4 Argon")]);
        assert!(unknown.is_empty());
    }

    #[test]
    fn a_prompt_answer_is_refused() {
        // `/usage` taken as a prompt (an agy without the command): tokens were spent.
        let raw = json!({"status": "SUCCESS", "num_turns": 1, "usage": {"total_tokens": 13532}, "response": "…"});
        assert_eq!(parse_usage(&raw), Err(UsageError::Unsupported));
        let mut spent = sample();
        spent["usage"]["total_tokens"] = json!(10);
        assert_eq!(parse_usage(&spent), Err(UsageError::Unsupported));
    }
}
