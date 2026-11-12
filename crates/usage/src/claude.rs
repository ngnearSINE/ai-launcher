//! Claude Desktop usage for exactly one profile folder.
//!
//! The folder's `config.json` names the signed-in account (`lastKnownAccountUuid`) and holds the
//! encrypted OAuth cache (`oauth:tokenCacheV2`). Only that account's tokens for
//! `api.anthropic.com` are used. One account may belong to several organisations (team and
//! personal); each organisation has its own token and limits, so all of them are reported and
//! the UI decides which to show.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::crypto::decrypt_os_crypt;
use crate::model::*;

const API: &str = "https://api.anthropic.com/api/oauth";

/// `profile` answers per token (a token always belongs to one account and organisation), kept
/// in memory under a hash of the token so each refresh asks only for `usage`. The usage API is
/// tightly rate limited and shared with the Claude apps and other tools reading the same
/// account; halving the requests keeps the launcher out of their way.
static PROFILES: std::sync::LazyLock<parking_lot::Mutex<std::collections::HashMap<u64, Value>>> = std::sync::LazyLock::new(Default::default);

fn token_key(token: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    token.hash(&mut h);
    h.finish()
}
const HOST_MARK: &str = ":https://api.anthropic.com:";

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub token: String,
    pub expires_at_ms: i64,
    pub scopes: Vec<String>,
}

/// Signed-in account of a profile and its usable tokens (newest expiry first).
#[derive(Debug, Clone)]
pub struct ProfileSession {
    pub account_uuid: String,
    pub tokens: Vec<Token>,
}

/// Whether the folder looks like a Claude profile that has signed in at least once.
pub fn has_session(profile_dir: &Path) -> bool {
    std::fs::read(profile_dir.join("config.json"))
        .ok()
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .map(|c| c.get("oauth:tokenCacheV2").and_then(Value::as_str).is_some())
        .unwrap_or(false)
}

/// Reads and decrypts the session of `profile_dir` only.
pub fn read_session(profile_dir: &Path, now_ms: i64) -> Result<ProfileSession, UsageError> {
    let config = std::fs::read(profile_dir.join("config.json")).map_err(|_| UsageError::NotSignedIn)?;
    let config: Value = serde_json::from_slice(&config).map_err(|_| UsageError::Format("config.json".into()))?;
    let encoded = config.get("oauth:tokenCacheV2").and_then(Value::as_str).ok_or(UsageError::NotSignedIn)?;
    let mut plain = decrypt_os_crypt(profile_dir, encoded)?;
    let cache: Result<Value, _> = serde_json::from_slice(&plain);
    plain.iter_mut().for_each(|b| *b = 0);
    let cache = cache.map_err(|_| UsageError::Decrypt("json".into()))?;
    let declared = config
        .get("lastKnownAccountUuid")
        .or_else(|| config.get("activeAccountId"))
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    select_session(&cache, declared.as_deref(), now_ms)
}

/// Picks the declared account's tokens. Without a declaration the cache must hold exactly one
/// account; otherwise the profile is ambiguous and nothing is used.
pub fn select_session(cache: &Value, declared: Option<&str>, now_ms: i64) -> Result<ProfileSession, UsageError> {
    let obj = cache.as_object().ok_or(UsageError::NotSignedIn)?;
    let mut by_account: BTreeMap<String, (Vec<Token>, bool)> = BTreeMap::new();
    for (key, raw) in obj {
        let Some(rest) = key.strip_prefix("acct:") else { continue };
        let Some((account, tail)) = rest.split_once('|') else { continue };
        let Some(pos) = tail.find(HOST_MARK) else { continue };
        let scopes: Vec<String> = tail[pos + HOST_MARK.len()..].split(' ').map(str::to_string).collect();
        let entry = by_account.entry(account.to_lowercase()).or_default();
        let token = raw.get("token").and_then(Value::as_str).unwrap_or_default();
        let expires = raw.get("expiresAt").and_then(Value::as_i64).unwrap_or(0);
        if token.is_empty() {
            continue;
        }
        if expires <= now_ms {
            entry.1 = true; // had something, but expired
            continue;
        }
        if scopes.iter().any(|s| s == "user:profile") {
            entry.0.push(Token { token: token.to_string(), expires_at_ms: expires, scopes });
        }
    }
    let account = match declared {
        Some(d) => d.to_lowercase(),
        None if by_account.len() == 1 => by_account.keys().next().cloned().unwrap_or_default(),
        None if by_account.is_empty() => return Err(UsageError::NotSignedIn),
        None => return Err(UsageError::AccountUnknown),
    };
    let (mut tokens, expired) = by_account.remove(&account).unwrap_or_default();
    if tokens.is_empty() {
        return Err(if expired { UsageError::SessionExpired } else { UsageError::NotSignedIn });
    }
    tokens.sort_by(|a, b| b.expires_at_ms.cmp(&a.expires_at_ms));
    tokens.dedup_by(|a, b| a.token == b.token);
    Ok(ProfileSession { account_uuid: account, tokens })
}

/// `Max 20x`, `Team`, `Pro`… from organisation type and rate-limit tier.
pub fn plan_label(org_type: &str, tier: Option<&str>) -> String {
    let t = tier.unwrap_or_default().to_lowercase();
    let mult = t.split(|c: char| !c.is_ascii_alphanumeric()).find_map(|w| w.strip_suffix('x').filter(|n| n.chars().all(|c| c.is_ascii_digit()) && !n.is_empty()));
    let with = |base: &str| mult.map(|m| format!("{base} {m}x")).unwrap_or_else(|| base.to_string());
    match org_type {
        "claude_team" => with("Team"),
        "claude_enterprise" => "Enterprise".into(),
        _ if t.contains("max") || org_type == "claude_max" => with("Max"),
        "claude_pro" => "Pro".into(),
        _ if t.contains("pro") => "Pro".into(),
        "" => String::new(),
        other => {
            let s = other.trim_start_matches("claude_");
            let mut c = s.chars();
            c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
        }
    }
}

fn time(v: &Value) -> Option<DateTime<Utc>> {
    v.as_str().and_then(|s| DateTime::parse_from_rfc3339(s).ok()).map(|d| d.with_timezone(&Utc))
}

fn pct(v: &Value) -> Option<f64> {
    v.as_f64().filter(|p| p.is_finite()).map(|p| p.clamp(0.0, 100.0))
}

fn clean_label(s: &str) -> String {
    s.chars().filter(|c| !"<>&\"'`".contains(*c)).take(24).collect::<String>().trim().to_string()
}

const KNOWN_FIELDS: &[&str] = &[
    "five_hour",
    "seven_day",
    "seven_day_opus",
    "seven_day_sonnet",
    "seven_day_cowork",
    "seven_day_oauth_apps",
    "seven_day_breakdown",
    "limits",
    "extra_usage",
    "spend",
];

/// `{amount_minor, exponent, currency}` or a plain number (already major units) → (amount, currency).
fn money(v: &Value) -> Option<(f64, Option<String>)> {
    if let Some(n) = v.as_f64() {
        return Some((n, None));
    }
    let minor = v.get("amount_minor")?.as_f64()?;
    let exp = v.get("exponent").and_then(Value::as_i64).unwrap_or(2) as i32;
    Some((minor / 10f64.powi(exp), v.get("currency").and_then(Value::as_str).map(str::to_string)))
}

/// Pay-as-you-go credits: the newer `spend` object first, then the older `extra_usage`.
fn credits(obj: &serde_json::Map<String, Value>) -> Option<Quota> {
    if let Some(s) = obj.get("spend").filter(|s| s["enabled"].as_bool().unwrap_or(false)) {
        let used = money(&s["used"]);
        let limit = money(&s["limit"]).or_else(|| money(&s["cap"]));
        let p = pct(&s["percent"]).or_else(|| match (&used, &limit) {
            (Some((u, _)), Some((l, _))) if *l > 0.0 => Some((u / l * 100.0).clamp(0.0, 100.0)),
            _ => None,
        })?;
        let currency = used.as_ref().and_then(|u| u.1.clone()).or_else(|| limit.as_ref().and_then(|l| l.1.clone()));
        return Some(Quota {
            kind: QuotaKind::Credits,
            label: "credits".into(),
            used_percent: p,
            resets_at: None,
            used_amount: used.map(|u| u.0),
            limit_amount: limit.map(|l| l.0),
            currency,
        });
    }
    let x = obj.get("extra_usage").filter(|x| x["is_enabled"].as_bool().unwrap_or(false))?;
    let scale = 10f64.powi(x["decimal_places"].as_i64().unwrap_or(0) as i32);
    let used = x["used_credits"].as_f64().map(|v| v / scale);
    let limit = x["monthly_limit"].as_f64().map(|v| v / scale);
    let p = pct(&x["utilization"]).or_else(|| match (used, limit) {
        (Some(u), Some(l)) if l > 0.0 => Some((u / l * 100.0).clamp(0.0, 100.0)),
        _ => None,
    })?;
    Some(Quota {
        kind: QuotaKind::Credits,
        label: "credits".into(),
        used_percent: p,
        resets_at: None,
        used_amount: used,
        limit_amount: limit,
        currency: x["currency"].as_str().map(str::to_string),
    })
}

/// Parses `/api/oauth/usage` into quotas, the weekly breakdown and the names of fields not shown.
/// Unknown windows are skipped rather than failing the card.
pub fn parse_usage(raw: &Value) -> Result<(Vec<Quota>, Vec<Share>, Vec<String>), UsageError> {
    let obj = raw.as_object().ok_or(UsageError::Format("usage".into()))?;
    let mut quotas = Vec::new();
    let window = |key: &str, kind: QuotaKind, label: &str| -> Option<Quota> {
        let w = obj.get(key)?;
        Some(Quota {
            kind,
            label: label.into(),
            used_percent: pct(w.get("utilization")?)?,
            resets_at: time(&w["resets_at"]),
            used_amount: None,
            limit_amount: None,
            currency: None,
        })
    };
    quotas.extend(window("five_hour", QuotaKind::FiveHour, "5h"));
    quotas.extend(window("seven_day", QuotaKind::Weekly, "week"));
    quotas.extend(window("seven_day_opus", QuotaKind::Scoped, "Opus"));
    quotas.extend(window("seven_day_sonnet", QuotaKind::Scoped, "Sonnet"));
    quotas.extend(window("seven_day_cowork", QuotaKind::Scoped, "Cowork"));
    if let Some(limits) = obj.get("limits").and_then(Value::as_array) {
        for l in limits {
            if l["kind"] != "weekly_scoped" {
                continue;
            }
            let name = l["scope"]["model"]["display_name"].as_str().or(l["scope"]["surface"]["display_name"].as_str()).map(clean_label).unwrap_or_default();
            let Some(p) = pct(&l["percent"]) else { continue };
            if name.is_empty() || quotas.iter().any(|q| q.label.eq_ignore_ascii_case(&name)) {
                continue;
            }
            quotas.push(Quota {
                kind: QuotaKind::Scoped,
                label: name,
                used_percent: p,
                resets_at: time(&l["resets_at"]),
                used_amount: None,
                limit_amount: None,
                currency: None,
            });
        }
    }
    quotas.extend(credits(obj));
    let breakdown = obj
        .get("seven_day_breakdown")
        .and_then(|b| b["rows"].as_array())
        .map(|rows| {
            rows.iter()
                .filter_map(|r| {
                    let label = clean_label(r["display_name"].as_str()?);
                    Some(Share { label, percent: pct(&r["percent"])? })
                })
                .collect()
        })
        .unwrap_or_default();
    // Only fields that carry data are worth reporting as "not shown".
    let unknown = obj
        .iter()
        .filter(|(k, v)| !KNOWN_FIELDS.contains(&k.as_str()) && !v.is_null() && v.as_bool() != Some(false))
        .map(|(k, _)| k.clone())
        .collect();
    Ok((quotas, breakdown, unknown))
}

async fn get(client: &reqwest::Client, path: &str, token: &str) -> Result<Value, UsageError> {
    let res = client
        .get(format!("{API}/{path}"))
        .bearer_auth(token)
        .header("anthropic-beta", "oauth-2025-04-20")
        .header("accept", "application/json")
        .timeout(Duration::from_secs(8))
        .send()
        .await
        .map_err(|e| UsageError::Network(e.without_url().to_string()))?;
    let status = res.status().as_u16();
    if status == 429 {
        let wait = res.headers().get("retry-after").and_then(|v| v.to_str().ok()).and_then(|v| v.parse::<u64>().ok()).unwrap_or(300);
        return Err(UsageError::RateLimited(wait));
    }
    if !(200..300).contains(&status) {
        return Err(UsageError::Http(status));
    }
    res.json::<Value>().await.map_err(|_| UsageError::Format(path.into()))
}

/// Usage of every organisation the profile's signed-in account holds a token for.
pub async fn fetch(client: &reqwest::Client, profile_dir: &Path) -> Result<UsageReport, UsageError> {
    let now = Utc::now();
    let session = read_session(profile_dir, now.timestamp_millis())?;
    let mut orgs: Vec<OrgUsage> = Vec::new();
    let mut email = String::new();
    let mut last_err = None;
    for t in &session.tokens {
        let key = token_key(&t.token);
        let cached = PROFILES.lock().get(&key).cloned();
        let profile = match cached {
            Some(p) => p,
            None => match get(client, "profile", &t.token).await {
                Ok(p) => {
                    PROFILES.lock().insert(key, p.clone());
                    p
                }
                Err(e @ UsageError::RateLimited(_)) => return Err(e),
                Err(e) => {
                    last_err = Some(e);
                    continue;
                }
            },
        };
        // The token must belong to the account this profile declares.
        let acct = profile["account"]["uuid"].as_str().unwrap_or_default().to_lowercase();
        if !acct.is_empty() && acct != session.account_uuid {
            continue;
        }
        let org = &profile["organization"];
        let org_uuid = org["uuid"].as_str().unwrap_or_default().to_string();
        if org_uuid.is_empty() || orgs.iter().any(|o| o.org_uuid == org_uuid) {
            continue;
        }
        if email.is_empty() {
            email = profile["account"]["email"].as_str().unwrap_or_default().to_string();
        }
        let usage = match get(client, "usage", &t.token).await {
            Ok(u) => u,
            Err(e @ UsageError::RateLimited(_)) => return Err(e),
            Err(e) => {
                last_err = Some(e);
                continue;
            }
        };
        let (quotas, breakdown, unknown_fields) = parse_usage(&usage)?;
        let org_type = org["organization_type"].as_str().unwrap_or_default().to_string();
        orgs.push(OrgUsage {
            plan: plan_label(&org_type, org["rate_limit_tier"].as_str()),
            org_uuid,
            org_name: org["name"].as_str().unwrap_or_default().to_string(),
            org_type,
            quotas,
            breakdown,
            unknown_fields,
        });
    }
    if orgs.is_empty() {
        return Err(last_err.unwrap_or(UsageError::NotSignedIn));
    }
    // Team/enterprise first, then by name.
    orgs.sort_by_key(|o| (!matches!(o.org_type.as_str(), "claude_team" | "claude_enterprise"), o.org_name.to_lowercase()));
    Ok(UsageReport { account_uuid: session.account_uuid, email, orgs, fetched_at: now })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const H: &str = ":https://api.anthropic.com:user:profile user:inference";

    #[test]
    fn uses_only_declared_account() {
        let cache = json!({
            format!("acct:AAA|client{H}"): {"token": "a1", "expiresAt": 2000},
            format!("acct:BBB|client{H}"): {"token": "b1", "expiresAt": 3000},
            "acct:AAA|client:https://evil.example:user:profile": {"token": "x", "expiresAt": 9000},
        });
        let s = select_session(&cache, Some("aaa"), 1000).unwrap();
        assert_eq!(s.tokens.len(), 1);
        assert_eq!(s.tokens[0].token, "a1");
        assert_eq!(select_session(&cache, None, 1000).unwrap_err(), UsageError::AccountUnknown);
        assert_eq!(select_session(&cache, Some("aaa"), 5000).unwrap_err(), UsageError::SessionExpired);
        assert_eq!(select_session(&cache, Some("ccc"), 1000).unwrap_err(), UsageError::NotSignedIn);
    }

    #[test]
    fn single_account_without_declaration() {
        let cache = json!({ format!("acct:AAA|c{H}"): {"token": "a1", "expiresAt": 2000} });
        assert_eq!(select_session(&cache, None, 1000).unwrap().account_uuid, "aaa");
    }

    #[test]
    fn parses_windows_scoped_and_credits() {
        let raw = json!({
            "five_hour": {"utilization": 12.0, "resets_at": "2026-11-06T05:00:00+00:00"},
            "seven_day": {"utilization": 40.5, "resets_at": null},
            "seven_day_opus": null,
            "limits": [{"kind": "weekly_scoped", "percent": 3, "scope": {"model": {"display_name": "Fable"}}}],
            "extra_usage": {"is_enabled": true, "monthly_limit": 5000, "used_credits": 1250, "utilization": null, "decimal_places": 2, "currency": "USD"},
            "spend": {"enabled": false, "percent": 0},
            "seven_day_breakdown": {"rows": [{"display_name": "Claude Code", "percent": 100}, {"display_name": "Chats", "percent": 0}]},
            "something_new": 1,
            "empty_flag": null
        });
        let (q, breakdown, unknown) = parse_usage(&raw).unwrap();
        assert_eq!(q.iter().map(|q| q.label.as_str()).collect::<Vec<_>>(), ["5h", "week", "Fable", "credits"]);
        assert_eq!(q[3].used_percent, 25.0);
        assert_eq!(q[3].limit_amount, Some(50.0));
        assert_eq!(breakdown.len(), 2);
        assert_eq!(unknown, ["something_new"]);
    }

    #[test]
    fn spend_takes_precedence() {
        let raw = json!({
            "spend": {"enabled": true, "percent": 40, "used": {"amount_minor": 2000, "currency": "USD", "exponent": 2}, "limit": {"amount_minor": 5000, "currency": "USD", "exponent": 2}},
            "extra_usage": {"is_enabled": true, "utilization": 99}
        });
        let (q, _, _) = parse_usage(&raw).unwrap();
        assert_eq!(q.len(), 1);
        assert_eq!((q[0].used_percent, q[0].used_amount, q[0].limit_amount), (40.0, Some(20.0), Some(50.0)));
        assert_eq!(q[0].currency.as_deref(), Some("USD"));
    }

    #[test]
    fn plan_labels() {
        assert_eq!(plan_label("claude_team", Some("default_claude_team_5x")), "Team 5x");
        assert_eq!(plan_label("claude_max", Some("default_claude_max_20x")), "Max 20x");
        assert_eq!(plan_label("claude_pro", None), "Pro");
    }
}
