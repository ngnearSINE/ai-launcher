//! Grok usage through the Grok CLI's sign-in (`~/.grok/auth.json`, or `GROK_HOME`).
//!
//! The CLI keeps a short-lived access token (about six hours) next to a refresh token that
//! rotates on every use. The launcher only ever reads the access token: refreshing it here would
//! spend the CLI's refresh token and sign the CLI out. When the token has expired the last
//! numbers stay on screen (marked stale) until the CLI — or anything that runs it — refreshes it.
//!
//! Two read-only calls to the CLI's billing endpoint: `?format=credits` (the weekly credit pool
//! shared by Grok's apps) and the plain one (a monthly allowance, when the plan has one).

use std::path::PathBuf;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::cli::{self, CliAccount};
use crate::model::*;

const BILLING: &str = "https://cli-chat-proxy.grok.com/v1/billing";

pub fn auth_path() -> Option<PathBuf> {
    cli::dir_from_env("GROK_HOME", ".grok").map(|d| d.join("auth.json"))
}

/// The signed-in account. The token only ever lives in memory.
#[derive(Clone)]
pub struct Session {
    pub token: String,
    pub email: String,
    pub user_id: String,
    pub expires_at: Option<DateTime<Utc>>,
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session").field("user_id", &self.user_id).finish_non_exhaustive()
    }
}

/// The first sign-in in `auth.json` (`{"<issuer>::<client>": {key, email, user_id, expires_at…}}`).
pub fn session_from(raw: &Value) -> Option<Session> {
    raw.as_object()?.values().find_map(|e| {
        let token = e["key"].as_str().filter(|k| !k.is_empty())?;
        Some(Session {
            token: token.to_string(),
            email: e["email"].as_str().unwrap_or_default().to_string(),
            user_id: e["user_id"].as_str().or(e["principal_id"].as_str()).unwrap_or_default().to_string(),
            expires_at: e["expires_at"].as_str().and_then(|s| DateTime::parse_from_rfc3339(s).ok()).map(|t| t.with_timezone(&Utc)),
        })
    })
}

fn read_session() -> Result<Session, UsageError> {
    let bytes = auth_path().and_then(|p| std::fs::read(p).ok()).ok_or(UsageError::NotSignedIn)?;
    let raw: Value = serde_json::from_slice(&bytes).map_err(|_| UsageError::Format("grok auth.json".into()))?;
    session_from(&raw).ok_or(UsageError::NotSignedIn)
}

/// The account the CLI is signed in to, from its file alone (no request).
pub fn account() -> Option<CliAccount> {
    let s = read_session().ok()?;
    Some(CliAccount { email: s.email, plan: String::new() })
}

fn time(v: &Value) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(v.as_str()?).ok().map(|t| t.with_timezone(&Utc))
}

fn num(v: &Value) -> Option<f64> {
    v.as_f64().or_else(|| v.as_str().and_then(|s| s.parse().ok())).filter(|n| n.is_finite())
}

/// Fields of `billing?format=credits` that are read, or deliberately not shown.
const CREDIT_FIELDS: &[&str] = &[
    "currentPeriod",
    "creditUsagePercent",
    "productUsage",
    "billingPeriodStart",
    "billingPeriodEnd",
    "isUnifiedBillingUser",
    "prepaidBalance",
    "topUpMethod",
];

/// `billing?format=credits` → (quota, share per product, fields not shown).
///
/// Measured 2026-11-16: `{config: {currentPeriod: {type: "USAGE_PERIOD_TYPE_WEEKLY", start, end},
/// creditUsagePercent: 9.0, productUsage: [{product: "GrokBot", usagePercent: 8.0},
/// {product: "GrokBuild", usagePercent: 1.0}], billingPeriodStart, billingPeriodEnd, …}}`. A
/// missing `creditUsagePercent` means nothing used yet. Product percentages are of the whole
/// allowance, so they are turned into shares of what was used.
pub fn parse_credits(raw: &Value) -> Result<(Quota, Vec<Share>, Vec<String>), UsageError> {
    let c = &raw["config"];
    let obj = c.as_object().ok_or(UsageError::Format("grok credits".into()))?;
    let monthly = c["currentPeriod"]["type"].as_str().is_some_and(|t| t.contains("MONTH"));
    let quota = Quota {
        kind: if monthly { QuotaKind::Monthly } else { QuotaKind::Weekly },
        label: if monthly { "month" } else { "week" }.into(),
        used_percent: num(&c["creditUsagePercent"]).unwrap_or(0.0).clamp(0.0, 100.0),
        resets_at: time(&c["currentPeriod"]["end"]).or_else(|| time(&c["billingPeriodEnd"])),
        used_amount: None,
        limit_amount: None,
        currency: None,
    };
    let parts: Vec<(String, f64)> = c["productUsage"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|p| Some((p["product"].as_str()?.to_string(), num(&p["usagePercent"])?.max(0.0))))
        .collect();
    let total: f64 = parts.iter().map(|(_, p)| p).sum();
    let shares = if total > 0.0 {
        parts.into_iter().map(|(label, p)| Share { label: product_label(&label), percent: p / total * 100.0 }).collect()
    } else {
        Vec::new()
    };
    let unknown = obj.keys().filter(|k| !CREDIT_FIELDS.contains(&k.as_str())).cloned().collect();
    Ok((quota, shares, unknown))
}

/// `GrokBot` → `Grok Bot`, `GrokBuild` → `Grok Build`.
fn product_label(raw: &str) -> String {
    let mut out = String::new();
    for (i, ch) in raw.chars().enumerate() {
        if i > 0 && ch.is_uppercase() {
            out.push(' ');
        }
        out.push(ch);
    }
    out
}

/// Plain `billing` → the monthly allowance, when the plan has one (`monthlyLimit.val` > 0).
///
/// Measured 2026-11-16: `{config: {monthlyLimit: {val}, used: {val}, onDemandCap: {val},
/// billingPeriodStart, billingPeriodEnd, history: […]}}` (all zero on a credits plan).
pub fn parse_monthly(raw: &Value) -> Option<Quota> {
    let c = &raw["config"];
    let limit = num(&c["monthlyLimit"]["val"]).filter(|l| *l > 0.0)?;
    let used = num(&c["used"]["val"]).unwrap_or(0.0).max(0.0);
    Some(Quota {
        kind: QuotaKind::Monthly,
        label: "month".into(),
        used_percent: (used / limit * 100.0).clamp(0.0, 100.0),
        resets_at: time(&c["billingPeriodEnd"]),
        used_amount: None,
        limit_amount: None,
        currency: None,
    })
}

async fn get(client: &reqwest::Client, s: &Session, url: &str) -> Result<Value, UsageError> {
    let res = client
        .get(url)
        .bearer_auth(&s.token)
        .header("x-xai-token-auth", "xai-grok-cli")
        .header("accept", "application/json")
        .timeout(Duration::from_secs(8))
        .send()
        .await
        .map_err(|e| UsageError::Network(e.without_url().to_string()))?;
    match res.status().as_u16() {
        429 => {
            let wait = res.headers().get("retry-after").and_then(|v| v.to_str().ok()).and_then(|v| v.parse().ok()).unwrap_or(300);
            Err(UsageError::RateLimited(wait))
        }
        401 | 403 => Err(UsageError::SessionExpired),
        s if !(200..300).contains(&s) => Err(UsageError::Http(s)),
        _ => res.json::<Value>().await.map_err(|_| UsageError::Format("grok billing".into())),
    }
}

/// Usage of the account the Grok CLI is signed in to.
pub async fn fetch(client: &reqwest::Client) -> Result<UsageReport, UsageError> {
    let now = Utc::now();
    let s = read_session()?;
    if s.expires_at.is_some_and(|t| t < now + chrono::Duration::seconds(30)) {
        return Err(UsageError::SessionExpired);
    }
    let credits_url = format!("{BILLING}?format=credits");
    let (credits, monthly) = tokio::join!(get(client, &s, &credits_url), get(client, &s, BILLING));
    let (quota, breakdown, unknown_fields) = parse_credits(&credits?)?;
    let mut quotas = vec![quota];
    // The monthly allowance is extra: its failure does not hide the weekly numbers.
    quotas.extend(monthly.ok().as_ref().and_then(parse_monthly).filter(|_| !quotas.iter().any(|q| q.kind == QuotaKind::Monthly)));
    let id = if s.user_id.is_empty() { format!("grok:{}", s.email) } else { s.user_id.clone() };
    let org = OrgUsage {
        org_uuid: id.clone(),
        org_name: String::new(),
        org_type: "grok-cli".into(),
        plan: String::new(),
        quotas,
        breakdown,
        unknown_fields,
    };
    Ok(UsageReport { account_uuid: id, email: s.email, orgs: vec![org], fetched_at: now })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn weekly_credits_with_product_shares() {
        let raw = json!({"config": {
            "currentPeriod": {"type": "USAGE_PERIOD_TYPE_WEEKLY", "start": "2026-11-12T02:14:00.181+00:00", "end": "2026-11-19T02:14:00.181+00:00"},
            "creditUsagePercent": 9.0,
            "productUsage": [{"product": "GrokBot", "usagePercent": 8.0}, {"product": "GrokBuild", "usagePercent": 1.0}],
            "isUnifiedBillingUser": true,
            "newThing": 1
        }});
        let (q, shares, unknown) = parse_credits(&raw).unwrap();
        assert_eq!(q.kind, QuotaKind::Weekly);
        assert_eq!(q.used_percent, 9.0);
        assert_eq!(q.resets_at.unwrap().to_rfc3339(), "2026-11-19T02:14:00.181+00:00");
        assert_eq!(shares[0].label, "Grok Bot");
        assert!((shares[0].percent - 800.0 / 9.0).abs() < 1e-9);
        assert_eq!(unknown, vec!["newThing"]);
    }

    #[test]
    fn missing_percent_is_nothing_used() {
        let raw = json!({"config": {"currentPeriod": {"type": "USAGE_PERIOD_TYPE_WEEKLY"}, "billingPeriodEnd": "2026-11-19T02:14:00Z"}});
        let (q, shares, _) = parse_credits(&raw).unwrap();
        assert_eq!(q.used_percent, 0.0);
        assert!(q.resets_at.is_some());
        assert!(shares.is_empty());
        assert!(parse_credits(&json!({})).is_err());
    }

    #[test]
    fn monthly_only_with_a_limit() {
        let zero = json!({"config": {"monthlyLimit": {"val": 0}, "used": {"val": 0}}});
        assert!(parse_monthly(&zero).is_none());
        let some = json!({"config": {"monthlyLimit": {"val": 200}, "used": {"val": 50}, "billingPeriodEnd": "2026-12-01T00:00:00+00:00"}});
        let q = parse_monthly(&some).unwrap();
        assert_eq!((q.kind, q.used_percent), (QuotaKind::Monthly, 25.0));
    }

    #[test]
    fn reads_the_first_signed_in_entry() {
        let raw = json!({"https://auth.x.ai::client": {"key": "tok", "email": "a@example.com", "user_id": "u1", "expires_at": "2026-11-16T12:46:56.783Z", "refresh_token": "r"}});
        let s = session_from(&raw).unwrap();
        assert_eq!((s.email.as_str(), s.user_id.as_str()), ("a@example.com", "u1"));
        assert!(s.expires_at.is_some());
        assert!(session_from(&json!({"x": {"key": ""}})).is_none());
    }
}
