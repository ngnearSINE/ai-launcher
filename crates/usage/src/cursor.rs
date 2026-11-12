//! Cursor and Grok Bot usage for exactly one profile folder.
//!
//! Both apps sign in to a Cursor account and read their limits from the same (unofficial)
//! cursor.com dashboard API, so they share the caller here. Only the given folder is read:
//!
//! - **Cursor**: `User/globalStorage/state.vscdb` (SQLite, VS Code `ItemTable`) holds
//!   `cursorAuth/accessToken`. Current builds wrap it as `cursor-safe-storage:v1:<base64>`,
//!   the base64 being an Electron `safeStorage` (`v10`) blob; older builds kept the JWT as is.
//! - **Grok Bot**: `sand-secrets.json` → `cursor-accounts`, JSON naming the `active` account;
//!   each account's `cursor-access-token` is a `safeStorage` (`v10`) blob.
//!
//! The token is a JWT whose `sub` (`<provider>|<user id>`) names the account. The dashboard
//! API wants it as the `WorkosCursorSessionToken=<user id>::<token>` cookie.

use std::path::Path;
use std::time::Duration;

use base64::Engine;
use chrono::{DateTime, TimeZone, Utc};
use serde_json::Value;

use crate::crypto::decrypt_os_crypt;
use crate::model::*;

const BASE: &str = "https://cursor.com";
const SAFE_STORAGE: &str = "cursor-safe-storage:";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum App {
    Cursor,
    GrokBot,
}

/// The signed-in account of one folder. The token only ever lives in memory.
#[derive(Clone)]
pub struct Session {
    pub token: String,
    /// Account id from the token's `sub` (`user_…`), used as the report's account.
    pub user_id: String,
    pub email: String,
    /// Plan the app last saw (`pro`, `free`…), when it keeps one.
    pub plan_hint: String,
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session").field("user_id", &self.user_id).finish_non_exhaustive()
    }
}

fn state_db(dir: &Path) -> std::path::PathBuf {
    dir.join("User").join("globalStorage").join("state.vscdb")
}

/// Whether the folder holds a sign-in at all (no decryption, no network).
pub fn has_session(app: App, dir: &Path) -> bool {
    match app {
        App::Cursor => read_items(&state_db(dir), &["cursorAuth/accessToken"]).map(|v| !v[0].is_empty()).unwrap_or(false),
        App::GrokBot => grok_accounts(dir).is_some_and(|a| active_grok_account(&a).is_ok()),
    }
}

/// Values of `keys` in a VS Code `ItemTable` (empty string when missing). Read-only.
fn read_items(db: &Path, keys: &[&str]) -> Result<Vec<String>, UsageError> {
    use rusqlite::{Connection, OpenFlags, OptionalExtension};
    if !db.is_file() {
        return Err(UsageError::NotSignedIn);
    }
    let conn = Connection::open_with_flags(db, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)
        .map_err(|_| UsageError::Format("state.vscdb".into()))?;
    let _ = conn.busy_timeout(Duration::from_secs(2));
    let mut stmt = conn.prepare("SELECT value FROM ItemTable WHERE key = ?1").map_err(|_| UsageError::Format("state.vscdb".into()))?;
    keys.iter()
        .map(|k| {
            stmt.query_row([k], |r| {
                // VS Code writes TEXT, some versions BLOB.
                Ok(match r.get_ref(0)? {
                    rusqlite::types::ValueRef::Text(t) | rusqlite::types::ValueRef::Blob(t) => String::from_utf8_lossy(t).into_owned(),
                    _ => String::new(),
                })
            })
            .optional()
            .map(Option::unwrap_or_default)
            .map_err(|_| UsageError::Format("state.vscdb".into()))
        })
        .collect()
}

/// `cursor-safe-storage:v1:<base64 v10 blob>` → token; a bare token passes through.
fn unwrap_safe_storage(dir: &Path, stored: &str) -> Result<String, UsageError> {
    let stored = stored.trim().trim_matches('"');
    let Some(rest) = stored.strip_prefix(SAFE_STORAGE) else { return Ok(stored.to_string()) };
    let (_version, blob) = rest.split_once(':').ok_or(UsageError::Decrypt("envelope".into()))?;
    let plain = decrypt_os_crypt(dir, blob)?;
    String::from_utf8(plain).map_err(|_| UsageError::Decrypt("utf8".into()))
}

fn jwt_claims(token: &str) -> Option<Value> {
    let payload = token.split('.').nth(1)?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(payload.trim_end_matches('=')).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Builds the session from a token, refusing expired ones without asking the server.
fn session_from(token: String, email: String, plan_hint: String, now: DateTime<Utc>) -> Result<Session, UsageError> {
    if token.is_empty() {
        return Err(UsageError::NotSignedIn);
    }
    let claims = jwt_claims(&token).ok_or(UsageError::Format("token".into()))?;
    if claims["exp"].as_i64().is_some_and(|exp| exp <= now.timestamp()) {
        return Err(UsageError::SessionExpired);
    }
    let sub = claims["sub"].as_str().unwrap_or_default();
    let user_id = sub.rsplit('|').next().unwrap_or_default().to_string();
    if user_id.is_empty() {
        return Err(UsageError::Format("token".into()));
    }
    Ok(Session { token, user_id, email, plan_hint })
}

fn cursor_session(dir: &Path, now: DateTime<Utc>) -> Result<Session, UsageError> {
    let mut v = read_items(&state_db(dir), &["cursorAuth/accessToken", "cursorAuth/cachedEmail", "cursorAuth/stripeMembershipType"])?;
    if v[0].is_empty() {
        return Err(UsageError::NotSignedIn);
    }
    let token = unwrap_safe_storage(dir, &v[0]);
    v[0].clear();
    session_from(token?, std::mem::take(&mut v[1]), std::mem::take(&mut v[2]), now)
}

/// `sand-secrets.json` → `cursor-accounts`, a JSON text (not itself encrypted).
fn grok_accounts(dir: &Path) -> Option<Value> {
    let raw = std::fs::read(dir.join("sand-secrets.json")).ok()?;
    let v: Value = serde_json::from_slice(&raw).ok()?;
    serde_json::from_str(v["cursor-accounts"].as_str()?).ok()
}

/// `{"active": <id>, "accounts": {<id>: {"cursor-access-token": <v10 base64>, …}}}` → the
/// active account's entry. Without `active`, only a single account is unambiguous.
fn active_grok_account(accounts: &Value) -> Result<&Value, UsageError> {
    let list = accounts["accounts"].as_object().ok_or(UsageError::NotSignedIn)?;
    let signed_in = |a: &&Value| a["cursor-access-token"].as_str().is_some_and(|t| !t.is_empty());
    match accounts["active"].as_str().filter(|s| !s.is_empty()) {
        Some(id) => list.get(id).filter(signed_in).ok_or(UsageError::NotSignedIn),
        None => {
            let mut it = list.values().filter(signed_in);
            match (it.next(), it.next()) {
                (Some(one), None) => Ok(one),
                (None, _) => Err(UsageError::NotSignedIn),
                _ => Err(UsageError::AccountUnknown),
            }
        }
    }
}

fn grok_session(dir: &Path, now: DateTime<Utc>) -> Result<Session, UsageError> {
    let accounts = grok_accounts(dir).ok_or(UsageError::NotSignedIn)?;
    let account = active_grok_account(&accounts)?;
    let token = decrypt_os_crypt(dir, account["cursor-access-token"].as_str().unwrap_or_default())?;
    let token = String::from_utf8(token).map_err(|_| UsageError::Decrypt("utf8".into()))?;
    // The profile is optional: without it the card simply shows no address.
    let email = account["cursor-account-profile"]
        .as_str()
        .and_then(|p| decrypt_os_crypt(dir, p).ok())
        .and_then(|p| serde_json::from_slice::<Value>(&p).ok())
        .and_then(|p| p["email"].as_str().map(str::to_string))
        .unwrap_or_default();
    session_from(token, email, String::new(), now)
}

pub fn read_session(app: App, dir: &Path, now: DateTime<Utc>) -> Result<Session, UsageError> {
    match app {
        App::Cursor => cursor_session(dir, now),
        App::GrokBot => grok_session(dir, now),
    }
}

// ---- responses ---------------------------------------------------------------------------------

fn time(v: &Value) -> Option<DateTime<Utc>> {
    if let Some(s) = v.as_str() {
        if let Ok(d) = DateTime::parse_from_rfc3339(s) {
            return Some(d.with_timezone(&Utc));
        }
        if let Ok(n) = s.parse::<i64>() {
            return epoch(n);
        }
        return None;
    }
    v.as_i64().and_then(epoch)
}

/// Seconds or milliseconds since the epoch.
fn epoch(n: i64) -> Option<DateTime<Utc>> {
    let ms = if n > 100_000_000_000 { n } else { n * 1000 };
    Utc.timestamp_millis_opt(ms).single()
}

fn pct(v: &Value) -> Option<f64> {
    v.as_f64().filter(|p| p.is_finite()).map(|p| p.clamp(0.0, 100.0))
}

fn ratio(used: Option<f64>, limit: Option<f64>) -> Option<f64> {
    match (used, limit) {
        (Some(u), Some(l)) if l > 0.0 => Some((u / l * 100.0).clamp(0.0, 100.0)),
        _ => None,
    }
}

/// `pro`, `ultra`, `free_trial` → `Pro`, `Ultra`, `Free trial`.
pub fn plan_label(raw: &str) -> String {
    let s = raw.trim().replace(['_', '-'], " ");
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

fn unknown(obj: &serde_json::Map<String, Value>, known: &[&str]) -> Vec<String> {
    obj.iter()
        .filter(|(k, v)| !known.contains(&k.as_str()) && !v.is_null() && v.as_bool() != Some(false))
        .map(|(k, _)| k.clone())
        .collect()
}

/// Fields of `/api/usage-summary` that are read, or deliberately not shown (display texts).
const SUMMARY_FIELDS: &[&str] = &[
    "billingCycleStart",
    "billingCycleEnd",
    "membershipType",
    "individualUsage",
    "teamUsage",
    "isUnlimited",
    "limitType",
    "autoModelSelectedDisplayMessage",
    "namedModelSelectedDisplayMessage",
];

fn cents(v: &Value) -> Option<f64> {
    v.as_f64().filter(|c| c.is_finite()).map(|c| c / 100.0)
}

/// An on-demand (pay-as-you-go) block: `{enabled, used, limit, remaining}` in cents. Without a
/// limit the spend is unlimited, so it shows 0 % with the amount spent.
fn on_demand(v: &Value, label: &str, resets_at: Option<DateTime<Utc>>) -> Option<Quota> {
    if v["enabled"].as_bool() != Some(true) {
        return None;
    }
    let (used, limit) = (cents(&v["used"]), cents(&v["limit"]));
    Some(Quota {
        kind: QuotaKind::Credits,
        label: label.into(),
        used_percent: ratio(used, limit).unwrap_or(0.0),
        resets_at,
        used_amount: used,
        limit_amount: limit,
        currency: Some("USD".into()),
    })
}

/// `/api/usage-summary` → (plan, quotas, fields not shown).
///
/// Measured 2026-11-12: `{billingCycleStart, billingCycleEnd (RFC 3339), membershipType,
/// limitType, isUnlimited, individualUsage: {plan: {enabled, used, limit, remaining,
/// breakdown: {included, bonus, total}, autoPercentUsed, apiPercentUsed, totalPercentUsed},
/// onDemand: {enabled, used, limit, remaining}}, teamUsage: {}}`. Amounts are cents and
/// percentages 0–100; the plan percentage is `used / limit` and falls back to
/// `totalPercentUsed`, as Cursor's own status bar does.
pub fn parse_usage_summary(raw: &Value) -> Result<(String, Vec<Quota>, Vec<String>), UsageError> {
    let obj = raw.as_object().ok_or(UsageError::Format("usage-summary".into()))?;
    let ind = obj.get("individualUsage").filter(|v| v.is_object()).ok_or(UsageError::Format("usage-summary".into()))?;
    let cycle_end = time(&raw["billingCycleEnd"]);
    let mut quotas = Vec::new();
    let plan = &ind["plan"];
    if plan.is_object() && plan["enabled"].as_bool() != Some(false) && raw["isUnlimited"].as_bool() != Some(true) {
        let (used, limit) = (cents(&plan["used"]), cents(&plan["limit"]));
        if let Some(p) = ratio(used, limit).or_else(|| pct(&plan["totalPercentUsed"])) {
            quotas.push(Quota {
                kind: QuotaKind::Monthly,
                label: "plan".into(),
                used_percent: p,
                resets_at: cycle_end,
                used_amount: used,
                limit_amount: limit,
                currency: used.or(limit).map(|_| "USD".into()),
            });
        }
        // The two pools Cursor draws separately (Auto + Composer, and named API models), on the
        // same billing cycle as the plan (so the UI paces them by the month, not the week).
        for (key, label) in [("autoPercentUsed", "Auto"), ("apiPercentUsed", "API")] {
            if let Some(p) = pct(&plan[key]) {
                quotas.push(Quota {
                    kind: QuotaKind::Monthly,
                    label: label.into(),
                    used_percent: p,
                    resets_at: cycle_end,
                    used_amount: None,
                    limit_amount: None,
                    currency: None,
                });
            }
        }
    }
    quotas.extend(on_demand(&ind["onDemand"], "on-demand", cycle_end));
    quotas.extend(on_demand(&raw["teamUsage"]["onDemand"], "team on-demand", cycle_end));
    let plan_name = obj.get("membershipType").and_then(Value::as_str).map(plan_label).unwrap_or_default();
    Ok((plan_name, quotas, unknown(obj, SUMMARY_FIELDS)))
}

/// Fields of `get-sand-usage-status` that are read, or are links and upsell texts not shown.
const SAND_FIELDS: &[&str] = &[
    "usagePercent",
    "nextResetTimestampUtc",
    "currentPeriodStart",
    "isUnified",
    "grokPlanLabel",
    "cursorPlanName",
    "billingBrand",
    "hasAvailableUsage",
    "hasNonZeroIncludedLimit",
    "prepaidAvailableCents",
    "onDemandSettings",
    "unifiedBillingUrl",
    "unifiedUpsellButtons",
    "upgradeOption",
    "upgradeRecommendation",
    "upgradeRecommendations",
];

/// Plan names come from the service; keep them short and free of markup characters.
fn clean_label(s: &str) -> String {
    s.chars().filter(|c| !"<>&\"'`".contains(*c)).take(32).collect::<String>().trim().to_string()
}

/// `/api/dashboard/get-sand-usage-status` → (plan, quotas, fields not shown).
///
/// Measured 2026-11-12: `usagePercent` (0–100: Grok Bot itself clamps it to 0..100 and treats
/// < 100 as "usage left"), `nextResetTimestampUtc` and `currentPeriodStart` (RFC 3339, one
/// week apart), `grokPlanLabel`, `cursorPlanName`, `isUnified`, plus links and upsell texts.
pub fn parse_sand_status(raw: &Value) -> Result<(String, Vec<Quota>, Vec<String>), UsageError> {
    let obj = raw.as_object().ok_or(UsageError::Format("sand-usage".into()))?;
    let p = pct(&raw["usagePercent"]).ok_or(UsageError::Format("sand-usage".into()))?;
    let quota = Quota {
        kind: QuotaKind::Weekly,
        label: "week".into(),
        used_percent: p,
        resets_at: time(&raw["nextResetTimestampUtc"]),
        used_amount: None,
        limit_amount: None,
        currency: None,
    };
    let plan = ["grokPlanLabel", "cursorPlanName"]
        .iter()
        .find_map(|k| raw[*k].as_str().map(clean_label).filter(|s| !s.is_empty()))
        .unwrap_or_default();
    Ok((plan, vec![quota], unknown(obj, SAND_FIELDS)))
}

// ---- network -----------------------------------------------------------------------------------

fn cookie(s: &Session) -> String {
    format!("WorkosCursorSessionToken={}%3A%3A{}", s.user_id, s.token)
}

async fn call(req: reqwest::RequestBuilder, what: &str) -> Result<Value, UsageError> {
    let res = req
        .header("accept", "application/json")
        .timeout(Duration::from_secs(8))
        .send()
        .await
        .map_err(|e| UsageError::Network(e.without_url().to_string()))?;
    let status = res.status().as_u16();
    match status {
        429 => {
            let wait = res.headers().get("retry-after").and_then(|v| v.to_str().ok()).and_then(|v| v.parse::<u64>().ok()).unwrap_or(300);
            Err(UsageError::RateLimited(wait))
        }
        401 | 403 => Err(UsageError::SessionExpired),
        s if !(200..300).contains(&s) => Err(UsageError::Http(s)),
        _ => res.json::<Value>().await.map_err(|_| UsageError::Format(what.into())),
    }
}

/// Usage of the account signed in to `dir`, as one organisation.
pub async fn fetch(client: &reqwest::Client, app: App, dir: &Path) -> Result<UsageReport, UsageError> {
    let now = Utc::now();
    let s = read_session(app, dir, now)?;
    let (plan, quotas, unknown_fields, org_type) = match app {
        App::Cursor => {
            let req = client.get(format!("{BASE}/api/usage-summary")).header("cookie", cookie(&s));
            let raw = call(req, "usage-summary").await?;
            let (plan, quotas, unknown) = parse_usage_summary(&raw)?;
            let plan = if plan.is_empty() { plan_label(&s.plan_hint) } else { plan };
            (plan, quotas, unknown, "cursor")
        }
        App::GrokBot => {
            let req = client
                .post(format!("{BASE}/api/dashboard/get-sand-usage-status"))
                .header("cookie", cookie(&s))
                .header("origin", BASE)
                .json(&serde_json::json!({}));
            let raw = call(req, "sand-usage").await?;
            let (plan, quotas, unknown) = parse_sand_status(&raw)?;
            (plan, quotas, unknown, "grok-bot")
        }
    };
    let org = OrgUsage {
        org_uuid: s.user_id.clone(),
        org_name: String::new(),
        org_type: org_type.into(),
        plan,
        quotas,
        breakdown: Vec::new(),
        unknown_fields,
    };
    Ok(UsageReport { account_uuid: s.user_id.clone(), email: s.email.clone(), orgs: vec![org], fetched_at: now })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn jwt(claims: Value) -> String {
        let enc = |v: &[u8]| base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(v);
        format!("{}.{}.sig", enc(br#"{"alg":"HS256"}"#), enc(claims.to_string().as_bytes()))
    }

    #[test]
    fn parses_usage_summary() {
        let raw = json!({
            "billingCycleStart": "2026-11-01T00:00:00.000Z",
            "billingCycleEnd": "2026-12-01T00:00:00.000Z",
            "membershipType": "pro",
            "limitType": "user",
            "isUnlimited": false,
            "autoModelSelectedDisplayMessage": "text",
            "individualUsage": {
                "plan": {"enabled": true, "used": 500, "limit": 2000, "remaining": 1500,
                         "breakdown": {"included": 500, "bonus": 0, "total": 500},
                         "autoPercentUsed": 10, "apiPercentUsed": 40.5, "totalPercentUsed": 99},
                "onDemand": {"enabled": true, "used": 250, "limit": 1000, "remaining": 750}
            },
            "teamUsage": {},
            "somethingNew": 1
        });
        let (plan, q, unknown) = parse_usage_summary(&raw).unwrap();
        assert_eq!(plan, "Pro");
        assert_eq!(q.iter().map(|q| q.label.as_str()).collect::<Vec<_>>(), ["plan", "Auto", "API", "on-demand"]);
        // used / limit wins over totalPercentUsed, amounts are dollars.
        assert_eq!((q[0].kind.clone(), q[0].used_percent, q[0].used_amount, q[0].limit_amount), (QuotaKind::Monthly, 25.0, Some(5.0), Some(20.0)));
        assert_eq!(q[0].resets_at.unwrap().to_rfc3339(), "2026-12-01T00:00:00+00:00");
        assert_eq!((q[2].kind.clone(), q[2].used_percent), (QuotaKind::Monthly, 40.5));
        assert_eq!((q[3].kind.clone(), q[3].used_percent, q[3].limit_amount), (QuotaKind::Credits, 25.0, Some(10.0)));
        assert_eq!(unknown, ["somethingNew"]);
    }

    #[test]
    fn usage_summary_edge_cases() {
        // No limit → totalPercentUsed; on-demand off; unlimited on-demand shows the spend.
        let raw = json!({
            "membershipType": "free_trial",
            "individualUsage": {"plan": {"enabled": true, "used": 0, "limit": null, "totalPercentUsed": 12}, "onDemand": {"enabled": false, "used": 0, "limit": null}},
            "teamUsage": {"onDemand": {"enabled": true, "used": 1234, "limit": null}}
        });
        let (plan, q, _) = parse_usage_summary(&raw).unwrap();
        assert_eq!(plan, "Free trial");
        assert_eq!(q.len(), 2);
        assert_eq!((q[0].used_percent, q[0].resets_at), (12.0, None));
        assert_eq!((q[1].label.as_str(), q[1].used_percent, q[1].used_amount, q[1].limit_amount), ("team on-demand", 0.0, Some(12.34), None));
        // Unlimited plans have no plan bar.
        let (_, q, _) = parse_usage_summary(&json!({"isUnlimited": true, "individualUsage": {"plan": {"enabled": true, "used": 1, "limit": 2}}})).unwrap();
        assert!(q.is_empty());
        assert_eq!(parse_usage_summary(&json!({"error": "x"})).unwrap_err(), UsageError::Format("usage-summary".into()));
        assert_eq!(parse_usage_summary(&json!([])).unwrap_err(), UsageError::Format("usage-summary".into()));
    }

    #[test]
    fn parses_sand_status() {
        let raw = json!({
            "usagePercent": 37.25,
            "nextResetTimestampUtc": "2026-11-19T02:14:00.181Z",
            "currentPeriodStart": "2026-11-12T02:14:00.181Z",
            "isUnified": true,
            "grokPlanLabel": "Grok <b>Plan</b>",
            "cursorPlanName": "Pro",
            "hasAvailableUsage": true,
            "upgradeRecommendations": [{"kind": "x"}],
            "newThing": {"a": 1}
        });
        let (plan, q, unknown) = parse_sand_status(&raw).unwrap();
        assert_eq!(plan, "Grok bPlan/b");
        assert_eq!(q.len(), 1);
        assert_eq!((q[0].kind.clone(), q[0].label.as_str(), q[0].used_percent), (QuotaKind::Weekly, "week", 37.25));
        assert_eq!(q[0].resets_at.unwrap().to_rfc3339(), "2026-11-19T02:14:00.181+00:00");
        assert_eq!(unknown, ["newThing"]);
        // Over 100 is clamped; a missing percentage is a format error.
        assert_eq!(parse_sand_status(&json!({"usagePercent": 140})).unwrap().1[0].used_percent, 100.0);
        assert_eq!(parse_sand_status(&json!({"isUnified": true})).unwrap_err(), UsageError::Format("sand-usage".into()));
        // Falls back to the Cursor plan name.
        assert_eq!(parse_sand_status(&json!({"usagePercent": 1, "cursorPlanName": "Ultra"})).unwrap().0, "Ultra");
    }

    #[test]
    fn epoch_numbers_in_seconds_or_millis() {
        assert_eq!(time(&json!(1_763_518_440)).unwrap().timestamp(), 1_763_518_440);
        assert_eq!(time(&json!(1_763_518_440_181i64)).unwrap().timestamp_millis(), 1_763_518_440_181);
        assert_eq!(time(&json!("1763518440")).unwrap().timestamp(), 1_763_518_440);
        assert!(time(&json!("soon")).is_none());
    }

    #[test]
    fn session_from_token_claims() {
        let now = Utc.timestamp_opt(1_800_000_000, 0).unwrap();
        let t = jwt(json!({"sub": "provider|user_ABC", "exp": 1_900_000_000}));
        let s = session_from(t, "a@example.com".into(), "pro".into(), now).unwrap();
        assert_eq!(s.user_id, "user_ABC");
        assert!(!format!("{s:?}").contains(&s.token), "Debug must not print the token");
        assert_eq!(cookie(&s), format!("WorkosCursorSessionToken=user_ABC%3A%3A{}", s.token));
        let expired = jwt(json!({"sub": "p|user_ABC", "exp": 1_700_000_000}));
        assert_eq!(session_from(expired, String::new(), String::new(), now).unwrap_err(), UsageError::SessionExpired);
        assert_eq!(session_from(String::new(), String::new(), String::new(), now).unwrap_err(), UsageError::NotSignedIn);
        assert_eq!(session_from("opaque".into(), String::new(), String::new(), now).unwrap_err(), UsageError::Format("token".into()));
    }

    #[test]
    fn bare_cursor_token_passes_through() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(unwrap_safe_storage(dir.path(), "\"a.b.c\"").unwrap(), "a.b.c");
        // A wrapped token needs the folder's key; without `Local State` it cannot be read.
        assert!(matches!(unwrap_safe_storage(dir.path(), "cursor-safe-storage:v1:djEwAAAA"), Err(UsageError::Decrypt(_))));
    }

    #[test]
    fn picks_the_active_grok_account() {
        let accounts = json!({
            "active": "h2",
            "accounts": {
                "h1": {"cursor-access-token": "t1"},
                "h2": {"cursor-access-token": "t2", "cursor-account-profile": "p2"}
            }
        });
        assert_eq!(active_grok_account(&accounts).unwrap()["cursor-access-token"], "t2");
        let gone = json!({"active": "h3", "accounts": {"h1": {"cursor-access-token": "t1"}}});
        assert_eq!(active_grok_account(&gone).unwrap_err(), UsageError::NotSignedIn);
        let one = json!({"accounts": {"h1": {"cursor-access-token": "t1"}, "h2": {"cursor-access-token": ""}}});
        assert_eq!(active_grok_account(&one).unwrap()["cursor-access-token"], "t1");
        let two = json!({"accounts": {"h1": {"cursor-access-token": "t1"}, "h2": {"cursor-access-token": "t2"}}});
        assert_eq!(active_grok_account(&two).unwrap_err(), UsageError::AccountUnknown);
        assert_eq!(active_grok_account(&json!({})).unwrap_err(), UsageError::NotSignedIn);
    }

    #[test]
    fn reads_items_from_a_vscode_state_db() {
        let dir = tempfile::tempdir().unwrap();
        let db = state_db(dir.path());
        assert!(!has_session(App::Cursor, dir.path()));
        std::fs::create_dir_all(db.parent().unwrap()).unwrap();
        {
            let c = rusqlite::Connection::open(&db).unwrap();
            c.execute_batch("CREATE TABLE ItemTable (key TEXT UNIQUE ON CONFLICT REPLACE, value BLOB);").unwrap();
            c.execute("INSERT INTO ItemTable VALUES ('cursorAuth/cachedEmail', 'a@example.com')", []).unwrap();
        }
        assert!(!has_session(App::Cursor, dir.path()));
        assert_eq!(read_items(&db, &["cursorAuth/cachedEmail", "missing"]).unwrap(), ["a@example.com", ""]);
        assert_eq!(read_session(App::Cursor, dir.path(), Utc::now()).unwrap_err(), UsageError::NotSignedIn);
        rusqlite::Connection::open(&db).unwrap().execute("INSERT INTO ItemTable VALUES ('cursorAuth/accessToken', 'x.y.z')", []).unwrap();
        assert!(has_session(App::Cursor, dir.path()));
    }
}

