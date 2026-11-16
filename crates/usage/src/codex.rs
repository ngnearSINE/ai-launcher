//! Codex usage through the Codex CLI's own sign-in (`~/.codex`, or `CODEX_HOME`).
//!
//! Read the way OpenAI's editor extensions do: start `codex app-server`, ask it over its
//! line-delimited JSON-RPC (no `jsonrpc` field) for the account (`account/read`) and the rate
//! limits (`account/rateLimits/read`), then stop it. The launcher never touches the token; the
//! app server manages its own sign-in exactly as when Codex runs normally.
//!
//! These are the Codex (coding agent) limits of that ChatGPT account, not ChatGPT's chat limits.

use std::path::PathBuf;
use std::time::Duration;

use chrono::{DateTime, TimeZone, Utc};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::cli::{self, CliAccount};
use crate::model::*;

const TIMEOUT: Duration = Duration::from_secs(25);

pub fn codex_home() -> Option<PathBuf> {
    cli::dir_from_env("CODEX_HOME", ".codex")
}

/// The Codex program: `CODEX_EXECUTABLE`, else the newest of the desktop app's copies, the npm
/// package's native binary, `PATH` and the editor extension's copy.
pub fn find_exe() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("CODEX_EXECUTABLE").map(PathBuf::from).filter(|p| p.is_file()) {
        return Some(p);
    }
    let name = cli::exe_name("codex");
    let mut found = cli::on_path("codex");
    if let Some(local) = cli::local_app_data() {
        found.extend(cli::children(&local.join("OpenAI").join("Codex").join("bin")).into_iter().map(|d| d.join(&name)));
    }
    if let Some(roaming) = std::env::var_os("APPDATA").map(PathBuf::from) {
        let pkg = roaming.join("npm").join("node_modules").join("@openai").join("codex").join("node_modules").join("@openai");
        for arch in cli::children(&pkg) {
            for target in cli::children(&arch.join("vendor")) {
                found.push(target.join("bin").join(&name));
            }
        }
    }
    if let Some(home) = cli::home() {
        for ext in cli::children(&home.join(".vscode").join("extensions")) {
            let is_openai = ext.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("openai.chatgpt-"));
            if is_openai {
                found.extend(cli::children(&ext.join("bin")).into_iter().map(|d| d.join(&name)));
            }
        }
    }
    cli::newest(found)
}

/// The signed-in account as the CLI keeps it locally (`auth.json`'s id token), without asking
/// the service. None when Codex is not installed or never signed in.
pub fn account() -> Option<CliAccount> {
    find_exe()?;
    let raw: Value = serde_json::from_slice(&std::fs::read(codex_home()?.join("auth.json")).ok()?).ok()?;
    Some(account_from_auth(&raw))
}

/// Email and plan from `auth.json` (`tokens.id_token` claims). An API-key sign-in has neither.
pub fn account_from_auth(raw: &Value) -> CliAccount {
    let claims = raw["tokens"]["id_token"].as_str().and_then(crate::cursor::jwt_claims).unwrap_or(Value::Null);
    let email = claims["email"].as_str().or_else(|| claims["https://api.openai.com/profile"]["email"].as_str()).unwrap_or_default();
    let plan = claims["https://api.openai.com/auth"]["chatgpt_plan_type"].as_str().unwrap_or_default();
    CliAccount { email: email.to_string(), plan: cli::plan_label(plan) }
}

fn time(v: &Value) -> Option<DateTime<Utc>> {
    let s = v.as_f64()?;
    Utc.timestamp_opt(s as i64, 0).single()
}

/// One window `{usedPercent, windowDurationMins, resetsAt (epoch seconds)}`.
fn window(v: &Value, scoped: Option<&str>) -> Option<Quota> {
    let used = v["usedPercent"].as_f64().filter(|p| p.is_finite())?.clamp(0.0, 100.0);
    let mins = v["windowDurationMins"].as_f64().unwrap_or(0.0);
    let (kind, label) = match scoped {
        Some(name) => (QuotaKind::Scoped, name.to_string()),
        None if mins > 0.0 && mins < 24.0 * 60.0 => (QuotaKind::FiveHour, "5h".to_string()),
        None => (QuotaKind::Weekly, "week".to_string()),
    };
    Some(Quota { kind, label, used_percent: used, resets_at: time(&v["resetsAt"]), used_amount: None, limit_amount: None, currency: None })
}

/// Fields of the `account/rateLimits/read` result that are read, or deliberately not shown
/// (credit balances without a limit, reset vouchers, upsell).
const LIMIT_FIELDS: &[&str] = &[
    "rateLimits",
    "rateLimitsByLimitId",
    "ordinaryUsageAllowed",
    "rateLimitResetCredits",
    "accountId",
    "rateLimitUpsell",
];

/// `account/rateLimits/read` result → (quotas, fields not shown).
///
/// Measured 2026-11-16 (Codex 0.167, Pro): `{rateLimits: {limitId: "codex", primary:
/// {usedPercent, windowDurationMins: 10080, resetsAt}, secondary: null, credits, planType…},
/// rateLimitsByLimitId: {codex: {…same…}}, accountId, …}`. The primary window is the 5-hour
/// one when Codex has both; a plan with only a weekly limit reports it as `primary`. Other
/// limit ids (a model with its own allowance) show as their own weekly line.
pub fn parse_rate_limits(result: &Value) -> Result<(Vec<Quota>, Vec<String>), UsageError> {
    let obj = result.as_object().ok_or(UsageError::Format("rateLimits".into()))?;
    let by_id = result["rateLimitsByLimitId"].as_object();
    let main = result["rateLimits"]
        .as_object()
        .map(|_| &result["rateLimits"])
        .or_else(|| by_id.and_then(|m| m.values().next()))
        .ok_or(UsageError::Format("rateLimits".into()))?;
    let mut quotas: Vec<Quota> = ["primary", "secondary"].iter().filter_map(|k| window(&main[*k], None)).collect();
    quotas.sort_by_key(|q| if q.kind == QuotaKind::FiveHour { 0 } else { 1 });
    let main_id = main["limitId"].as_str().unwrap_or_default();
    for (id, other) in by_id.into_iter().flatten() {
        if id == main_id {
            continue;
        }
        let name = other["limitName"].as_str().filter(|s| !s.is_empty()).unwrap_or(id);
        quotas.extend(["primary", "secondary"].iter().filter_map(|k| window(&other[*k], Some(name))));
    }
    if quotas.is_empty() {
        return Err(UsageError::Format("rateLimits".into()));
    }
    let unknown = obj.keys().filter(|k| !LIMIT_FIELDS.contains(&k.as_str())).cloned().collect();
    Ok((quotas, unknown))
}

/// One JSON-RPC exchange with a running app server: the reply to `id`, skipping notifications.
async fn request<R: tokio::io::AsyncBufRead + Unpin>(
    stdin: &mut tokio::process::ChildStdin,
    lines: &mut tokio::io::Lines<R>,
    id: u64,
    method: &str,
    params: Value,
) -> Result<Value, UsageError> {
    let msg = json!({ "method": method, "id": id, "params": params });
    stdin.write_all(format!("{msg}\n").as_bytes()).await.map_err(|_| UsageError::Format("codex app-server".into()))?;
    stdin.flush().await.map_err(|_| UsageError::Format("codex app-server".into()))?;
    loop {
        let line = lines.next_line().await.map_err(|_| UsageError::Format("codex app-server".into()))?;
        let Some(line) = line else { return Err(UsageError::Format("codex app-server closed".into())) };
        let Ok(v) = serde_json::from_str::<Value>(&line) else { continue };
        if v["id"].as_u64() != Some(id) {
            continue;
        }
        if v.get("error").is_some_and(|e| !e.is_null()) {
            // Signed out (or the session could not be refreshed) shows up as an error reply.
            return Err(UsageError::SessionExpired);
        }
        return Ok(v["result"].clone());
    }
}

async fn exchange(exe: PathBuf) -> Result<UsageReport, UsageError> {
    let mut child = cli::command(&exe)
        .arg("app-server")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .map_err(|_| UsageError::Format("codex not runnable".into()))?;
    let mut stdin = child.stdin.take().ok_or(UsageError::Format("codex app-server".into()))?;
    let stdout = child.stdout.take().ok_or(UsageError::Format("codex app-server".into()))?;
    let mut lines = BufReader::new(stdout).lines();
    let version = env!("CARGO_PKG_VERSION");
    let init = json!({ "clientInfo": { "name": "ai_launcher", "title": "AI Launcher", "version": version }, "capabilities": { "experimentalApi": true } });
    let result = async {
        request(&mut stdin, &mut lines, 1, "initialize", init).await?;
        stdin.write_all(b"{\"method\":\"initialized\"}\n").await.map_err(|_| UsageError::Format("codex app-server".into()))?;
        let account = request(&mut stdin, &mut lines, 2, "account/read", json!({ "refreshToken": false })).await?;
        let acct = &account["account"];
        if !acct.is_object() {
            return Err(UsageError::NotSignedIn);
        }
        let limits = request(&mut stdin, &mut lines, 3, "account/rateLimits/read", json!({})).await?;
        let (quotas, unknown_fields) = parse_rate_limits(&limits)?;
        let email = acct["email"].as_str().unwrap_or_default().to_string();
        let plan = acct["planType"].as_str().or(limits["rateLimits"]["planType"].as_str()).unwrap_or_default();
        let id = limits["accountId"]
            .as_str()
            .or(account["workspaceRouting"]["chatgptAccountId"].as_str())
            .map(str::to_string)
            .unwrap_or_else(|| format!("codex:{email}"));
        let org = OrgUsage {
            org_uuid: id.clone(),
            org_name: String::new(),
            org_type: "codex-cli".into(),
            plan: cli::plan_label(plan),
            quotas,
            breakdown: Vec::new(),
            unknown_fields,
        };
        Ok(UsageReport { account_uuid: id, email, orgs: vec![org], fetched_at: Utc::now() })
    }
    .await;
    drop(stdin);
    let _ = child.kill().await;
    result
}

/// Usage of the account the Codex CLI is signed in to.
pub async fn fetch() -> Result<UsageReport, UsageError> {
    let exe = find_exe().ok_or(UsageError::NotSignedIn)?;
    if !codex_home().is_some_and(|h| h.join("auth.json").is_file()) {
        return Err(UsageError::NotSignedIn);
    }
    tokio::time::timeout(TIMEOUT, exchange(exe)).await.unwrap_or(Err(UsageError::Network("codex app-server timed out".into())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weekly_only_plan() {
        let raw = json!({
            "ordinaryUsageAllowed": true,
            "rateLimits": {"limitId": "codex", "primary": {"usedPercent": 12.5, "windowDurationMins": 10080, "resetsAt": 1795053673}, "secondary": null, "planType": "pro"},
            "rateLimitsByLimitId": {"codex": {"limitId": "codex", "primary": {"usedPercent": 12.5, "windowDurationMins": 10080, "resetsAt": 1795053673}}},
            "accountId": "acc",
            "somethingNew": 1
        });
        let (q, unknown) = parse_rate_limits(&raw).unwrap();
        assert_eq!(q.len(), 1);
        assert_eq!(q[0].kind, QuotaKind::Weekly);
        assert_eq!(q[0].used_percent, 12.5);
        assert_eq!(q[0].resets_at.unwrap().timestamp(), 1795053673);
        assert_eq!(unknown, vec!["somethingNew"]);
    }

    #[test]
    fn five_hour_first_and_other_limits_scoped() {
        let raw = json!({
            "rateLimits": {"limitId": "codex",
                "primary": {"usedPercent": 40, "windowDurationMins": 300, "resetsAt": 1795000000},
                "secondary": {"usedPercent": 10, "windowDurationMins": 10080, "resetsAt": 1795500000}},
            "rateLimitsByLimitId": {"codex_spark": {"limitId": "codex_spark", "limitName": "Spark", "primary": {"usedPercent": 3, "windowDurationMins": 10080}}}
        });
        let (q, _) = parse_rate_limits(&raw).unwrap();
        assert_eq!(q.iter().map(|q| q.kind.clone()).collect::<Vec<_>>(), [QuotaKind::FiveHour, QuotaKind::Weekly, QuotaKind::Scoped]);
        assert_eq!(q[2].label, "Spark");
    }

    #[test]
    fn falls_back_to_the_first_limit_id_and_rejects_empty() {
        let raw = json!({"rateLimitsByLimitId": {"codex": {"limitId": "codex", "primary": {"usedPercent": 1, "windowDurationMins": 300}}}});
        assert_eq!(parse_rate_limits(&raw).unwrap().0[0].kind, QuotaKind::FiveHour);
        assert!(parse_rate_limits(&json!({"rateLimits": {"primary": null}})).is_err());
    }

    #[test]
    fn account_from_id_token() {
        use base64::Engine;
        let enc = |v: &[u8]| base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(v);
        let claims = json!({"email": "a@example.com", "https://api.openai.com/auth": {"chatgpt_plan_type": "plus"}});
        let token = format!("{}.{}.sig", enc(b"{}"), enc(claims.to_string().as_bytes()));
        let a = account_from_auth(&json!({"tokens": {"id_token": token}}));
        assert_eq!(a, CliAccount { email: "a@example.com".into(), plan: "Plus".into() });
        assert_eq!(account_from_auth(&json!({"OPENAI_API_KEY": "x"})), CliAccount::default());
    }
}
