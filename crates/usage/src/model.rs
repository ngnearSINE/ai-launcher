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

/// Organisations an instance shows, as uuids in report order.
///
/// - The user's own pick wins (only the picked organisations that the report still has).
/// - Otherwise organisations that another instance signed in to the same account already
///   stands for are left out, so a "personal" instance set up after a "team" one shows the
///   personal organisation instead of repeating the team one. Another instance stands for an
///   organisation when the user picked it there, or when it is the only one that profile holds.
/// - If that would leave nothing, every organisation is shown.
pub fn shown_orgs(me: &UsageReport, pick: &[String], others: &[(&UsageReport, &[String])]) -> Vec<String> {
    let all = || me.orgs.iter().map(|o| o.org_uuid.clone());
    if !pick.is_empty() {
        let picked: Vec<String> = all().filter(|u| pick.contains(u)).collect();
        if !picked.is_empty() {
            return picked;
        }
    }
    let mut taken: Vec<&str> = Vec::new();
    for (r, p) in others {
        if r.account_uuid != me.account_uuid {
            continue;
        }
        let theirs: Vec<&str> = r.orgs.iter().map(|o| o.org_uuid.as_str()).collect();
        let picked: Vec<&str> = theirs.iter().copied().filter(|u| p.iter().any(|x| x == u)).collect();
        if !picked.is_empty() {
            taken.extend(picked);
        } else if theirs.len() == 1 {
            taken.push(theirs[0]);
        }
    }
    let free: Vec<String> = all().filter(|u| !taken.contains(&u.as_str())).collect();
    if free.is_empty() {
        all().collect()
    } else {
        free
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(account: &str, orgs: &[&str]) -> UsageReport {
        UsageReport {
            account_uuid: account.into(),
            email: String::new(),
            orgs: orgs
                .iter()
                .map(|u| OrgUsage {
                    org_uuid: (*u).into(),
                    org_name: (*u).into(),
                    org_type: String::new(),
                    plan: String::new(),
                    quotas: vec![],
                    breakdown: vec![],
                    unknown_fields: vec![],
                })
                .collect(),
            fetched_at: Utc::now(),
        }
    }

    #[test]
    fn leaves_out_what_another_instance_of_the_same_account_shows() {
        let team = report("a", &["team"]);
        let both = report("a", &["team", "personal"]);
        assert_eq!(shown_orgs(&both, &[], &[(&team, &[])]), vec!["personal"]);
        // The team-only instance keeps its one organisation.
        assert_eq!(shown_orgs(&team, &[], &[(&both, &[])]), vec!["team"]);
    }

    #[test]
    fn other_accounts_and_ambiguous_profiles_change_nothing() {
        let both = report("a", &["team", "personal"]);
        let other_account = report("b", &["team"]);
        assert_eq!(shown_orgs(&both, &[], &[(&other_account, &[])]), vec!["team", "personal"]);
        let both_too = report("a", &["team", "personal"]);
        assert_eq!(shown_orgs(&both, &[], &[(&both_too, &[])]), vec!["team", "personal"]);
    }

    #[test]
    fn picks_win_and_count_as_taken() {
        let both = report("a", &["team", "personal"]);
        let picked_personal = vec!["personal".to_string()];
        assert_eq!(shown_orgs(&both, &picked_personal, &[]), vec!["personal"]);
        // Another instance that picked "personal" leaves "team" for this one.
        assert_eq!(shown_orgs(&both, &[], &[(&both, &picked_personal)]), vec!["team"]);
        // A pick that no longer matches falls back to the automatic choice.
        assert_eq!(shown_orgs(&both, &["gone".to_string()], &[]), vec!["team", "personal"]);
    }

    #[test]
    fn never_shows_nothing() {
        let team = report("a", &["team"]);
        assert_eq!(shown_orgs(&team, &[], &[(&team, &[])]), vec!["team"]);
    }
}
