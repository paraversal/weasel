//! Invalid addresses: per-address flags, wildcard rules, and bounces.
//!
//! An address is *on hold* when it is flagged invalid, or when it matches a rule and
//! has not been explicitly marked valid. People whose current address is on hold stay
//! active but are left out of every send until their address changes or is cleared.

use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::{
    CoreError, CoreResult,
    email::EmailAddress,
    ports::{Clock, EmailCheckRepository},
    send::SendRecord,
};

/// Longest rule pattern accepted.
pub const PATTERN_MAX: usize = 200;
/// Addresses shown as examples when previewing a rule.
pub const PREVIEW_EXAMPLES: usize = 5;

crate::enums::string_enum! {
    /// A per-address verdict. `Valid` overrides any matching rule.
    pub enum EmailVerdict {
        Invalid => "invalid",
        Valid => "valid",
    }
}

crate::enums::string_enum! {
    /// Why an address was flagged by hand or by a bounce.
    pub enum FlagReason {
        Manual => "manual",
        Bounced => "bounced",
    }
}

/// Why an address is on hold.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum EmailHold {
    Manual,
    Bounced,
    Rule { pattern: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmailFlag {
    pub email: String,
    pub email_normalized: String,
    pub verdict: EmailVerdict,
    pub reason: FlagReason,
    pub created_at: i64,
    /// Active people whose current address this is.
    pub contacts: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InvalidRule {
    pub id: i64,
    pub pattern: String,
    pub created_at: i64,
    /// Active people whose current address matches, ignoring "marked valid" overrides.
    pub matches: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RulePreview {
    pub pattern: String,
    pub matches: u64,
    pub examples: Vec<String>,
}

/// Rules, flags and bounces. Patterns use `*` (any run of characters) and `?` (one
/// character) and match the whole lower-cased address.
pub struct EmailCheckService {
    repo: Arc<dyn EmailCheckRepository>,
    clock: Arc<dyn Clock>,
}

impl EmailCheckService {
    pub fn new(repo: Arc<dyn EmailCheckRepository>, clock: Arc<dyn Clock>) -> Self {
        Self { repo, clock }
    }

    pub async fn rules(&self) -> CoreResult<Vec<InvalidRule>> {
        self.repo.rules().await
    }

    pub async fn add_rule(&self, raw: &str) -> CoreResult<InvalidRule> {
        let pattern = clean_pattern(raw)?;
        self.repo.add_rule(&pattern, self.clock.now_ms()).await
    }

    pub async fn delete_rule(&self, id: i64) -> CoreResult<()> {
        self.repo.delete_rule(id, self.clock.now_ms()).await
    }

    pub async fn preview_rule(&self, raw: &str) -> CoreResult<RulePreview> {
        let pattern = clean_pattern(raw)?;
        self.repo.preview_rule(&pattern).await
    }

    pub async fn flags(&self) -> CoreResult<Vec<EmailFlag>> {
        self.repo.flags().await
    }

    /// Put an address on hold by hand.
    pub async fn mark_invalid(&self, raw: &str) -> CoreResult<()> {
        let email = EmailAddress::parse(raw)?;
        self.repo
            .set_flag(
                &email,
                EmailVerdict::Invalid,
                FlagReason::Manual,
                self.clock.now_ms(),
            )
            .await
    }

    /// Release an address: drops its flag, and overrides any rule that still matches.
    pub async fn mark_valid(&self, raw: &str) -> CoreResult<()> {
        let email = EmailAddress::parse(raw)?;
        self.repo.mark_valid(&email, self.clock.now_ms()).await
    }

    /// Remove a flag entirely, so rules apply to the address again.
    pub async fn clear_flag(&self, raw: &str) -> CoreResult<()> {
        let email = EmailAddress::parse(raw)?;
        self.repo.clear_flag(&email, self.clock.now_ms()).await
    }

    /// Holds for the given normalized addresses; addresses not on hold are absent.
    pub async fn holds(&self, normalized: &[String]) -> CoreResult<HashMap<String, EmailHold>> {
        if normalized.is_empty() {
            return Ok(HashMap::new());
        }
        self.repo.holds(normalized).await
    }

    /// Record (or undo) a bounce: the send record becomes `failed` and stops counting
    /// as received, and its address is flagged invalid. Undo restores both.
    pub async fn set_bounced(&self, record_id: i64, bounced: bool) -> CoreResult<SendRecord> {
        self.repo
            .set_bounced(record_id, bounced, self.clock.now_ms())
            .await
    }
}

/// Trim, lower-case and check a rule pattern.
pub fn clean_pattern(raw: &str) -> CoreResult<String> {
    let pattern = raw.trim().to_lowercase();
    if pattern.is_empty() {
        return Err(CoreError::validation("pattern", "is required"));
    }
    if pattern.chars().count() > PATTERN_MAX {
        return Err(CoreError::validation(
            "pattern",
            format!("is longer than {PATTERN_MAX} characters"),
        ));
    }
    if pattern.chars().any(char::is_whitespace) {
        return Err(CoreError::validation("pattern", "cannot contain spaces"));
    }
    if pattern.contains(['[', ']']) {
        return Err(CoreError::validation(
            "pattern",
            "can only use * and ? as wildcards",
        ));
    }
    let literal = pattern
        .chars()
        .filter(|ch| !matches!(ch, '*' | '?' | '@' | '.'))
        .count();
    if literal < 2 {
        return Err(CoreError::validation(
            "pattern",
            "would match almost every address; add part of a name or domain",
        ));
    }
    Ok(pattern)
}

/// Whole-string wildcard match with the same semantics as SQLite `GLOB` restricted to
/// `*` and `?`. Used by tests and by callers that check addresses without a database.
pub fn pattern_matches(pattern: &str, address: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    let address: Vec<char> = address.chars().collect();
    let (mut p, mut a) = (0, 0);
    let (mut star, mut mark) = (None, 0);
    while a < address.len() {
        if p < pattern.len() && (pattern[p] == '?' || pattern[p] == address[a]) {
            p += 1;
            a += 1;
        } else if p < pattern.len() && pattern[p] == '*' {
            star = Some(p);
            mark = a;
            p += 1;
        } else if let Some(position) = star {
            p = position + 1;
            mark += 1;
            a = mark;
        } else {
            return false;
        }
    }
    while p < pattern.len() && pattern[p] == '*' {
        p += 1;
    }
    p == pattern.len()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn patterns_are_trimmed_and_lower_cased() {
        assert_eq!(
            clean_pattern("  *@Provisional.Example.EDU ").unwrap(),
            "*@provisional.example.edu"
        );
    }

    #[test]
    fn patterns_that_match_everything_are_rejected() {
        for raw in ["*", "*@*", "*@*.*", "?*@*.?", ""] {
            assert!(clean_pattern(raw).is_err(), "{raw:?} should be rejected");
        }
    }

    #[test]
    fn brackets_and_spaces_are_rejected() {
        assert!(clean_pattern("[ab]@x.org").is_err());
        assert!(clean_pattern("a b@x.org").is_err());
    }

    #[test]
    fn wildcards_match_the_whole_address() {
        assert!(pattern_matches("*@temp.uni.edu", "ada@temp.uni.edu"));
        assert!(!pattern_matches("*@temp.uni.edu", "ada@temp.uni.edu.au"));
        assert!(pattern_matches("tmp-*@*", "tmp-42@uni.edu"));
        assert!(pattern_matches("a?a@x.org", "ada@x.org"));
        assert!(!pattern_matches("a?a@x.org", "adda@x.org"));
        assert!(pattern_matches("*.*@*", "ada.l@x.org"));
        assert!(!pattern_matches("*.*@*", "ada@x.org"));
    }
}
