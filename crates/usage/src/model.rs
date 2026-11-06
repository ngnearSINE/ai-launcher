use chrono::{DateTime, Utc};
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum QuotaKind {
    FiveHour,
    Weekly,
    /// A model- or surface-specific weekly allowance (e.g. Fable, Opus).
    Scoped,
    /// Pay-as-you-go credits on top of the plan.
    Credits,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Quota {
    pub kind: QuotaKind,
    pub label: String,
    /// 0–100.
    pub used_percent: f64,
    pub resets_at: Option<DateTime<Utc>>,
    /// Credits only: amounts in major units (e.g. dollars) and the currency code.
    pub used_amount: Option<f64>,
    pub limit_amount: Option<f64>,
    pub currency: Option<String>,
}

/// Share of the weekly usage per surface (Claude Code, chats, Cowork…).
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Share {
    pub label: String,
    pub percent: f64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OrgUsage {
    pub org_uuid: String,
    pub org_name: String,
    /// `claude_team`, `claude_max`, …
    pub org_type: String,
    /// Badge text such as `Max 20x`, `Team`, `Pro`.
    pub plan: String,
    pub quotas: Vec<Quota>,
    /// Weekly usage split by surface, when the service reports it.
    pub breakdown: Vec<Share>,
    /// Response fields this version does not display (names only, for diagnostics).
    pub unknown_fields: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UsageReport {
    pub account_uuid: String,
    pub email: String,
    pub orgs: Vec<OrgUsage>,
    pub fetched_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Error, Serialize, PartialEq)]
#[serde(tag = "code", content = "detail", rename_all = "camelCase")]
pub enum UsageError {
    /// The profile has never been signed in (no token cache).
    #[error("not signed in")]
    NotSignedIn,
    /// The profile does not say which account is signed in and holds several.
    #[error("account unknown")]
    AccountUnknown,
    /// Tokens exist for the account but all have expired; opening the app refreshes them.
    #[error("session expired")]
    SessionExpired,
    #[error("cannot decrypt: {0}")]
    Decrypt(String),
    #[error("rate limited for {0}s")]
    RateLimited(u64),
    #[error("http {0}")]
    Http(u16),
    #[error("network: {0}")]
    Network(String),
    #[error("unexpected response: {0}")]
    Format(String),
    #[error("not supported on this OS")]
    Unsupported,
}
