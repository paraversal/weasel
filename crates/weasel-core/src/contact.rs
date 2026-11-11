//! Contact domain types.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    email::EmailAddress,
    enums::{ContactSource, ContactStatus, EmailChangeReason},
    query::PageRequest,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Contact {
    pub id: i64,
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub email_normalized: String,
    pub identity_key: Option<String>,
    pub status: ContactStatus,
    pub source: ContactSource,
    pub import_batch_id: Option<i64>,
    pub date_added: i64,
    pub updated_at: i64,
    pub version: i64,
    pub notes: String,
    pub metadata: Value,
}

/// User input for a manual contact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewContact {
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub notes: String,
}

/// Validated row ready for the repository; services build this.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewContactRecord {
    pub first_name: String,
    pub last_name: String,
    pub email: EmailAddress,
    pub notes: String,
    pub source: ContactSource,
    pub identity_key: Option<String>,
}

/// Partial edit. `None` means "leave unchanged".
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContactPatch {
    pub version: i64,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub email: Option<String>,
    pub notes: Option<String>,
}

/// Validated changes ready for the repository.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContactChanges {
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub email: Option<EmailAddress>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ContactSort {
    #[default]
    LastNameAsc,
    LastNameDesc,
    AddedDesc,
    EmailAsc,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContactQuery {
    pub search: Option<String>,
    /// Empty means `[Active, Removed]` — archived people are opt-in.
    pub statuses: Vec<ContactStatus>,
    pub import_batch_id: Option<i64>,
    pub sort: ContactSort,
    pub page: PageRequest,
}

impl Default for ContactQuery {
    fn default() -> Self {
        Self {
            search: None,
            statuses: Vec::new(),
            import_batch_id: None,
            sort: ContactSort::LastNameAsc,
            page: PageRequest::default(),
        }
    }
}

impl ContactQuery {
    /// Invariant 8: a blank status filter hides archived contacts, never shows everything.
    pub fn effective_statuses(&self) -> Vec<ContactStatus> {
        if self.statuses.is_empty() {
            vec![ContactStatus::Active, ContactStatus::Removed]
        } else {
            self.statuses.clone()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmailHistoryEntry {
    pub email: String,
    pub valid_from: i64,
    pub valid_to: Option<i64>,
    pub reason: EmailChangeReason,
}

/// A campaign a contact was sent, from the immutable send snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CampaignReceipt {
    pub campaign_id: i64,
    pub campaign_name: String,
    pub batch_id: i64,
    pub sent_at: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContactDetail {
    pub contact: Contact,
    pub email_history: Vec<EmailHistoryEntry>,
    pub campaigns: Vec<CampaignReceipt>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchCount {
    pub batch_id: i64,
    pub source_name: String,
    pub count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContactStats {
    pub active: u64,
    pub removed: u64,
    pub archived: u64,
    pub total: u64,
    pub by_batch: Vec<BatchCount>,
}

/// Status changes are free-form; removed → active is a restore, archived → active an
/// unarchive. Same-status writes are not transitions.
pub fn can_transition(from: ContactStatus, to: ContactStatus) -> bool {
    from != to
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn blank_status_filter_excludes_archived() {
        let query = ContactQuery::default();
        assert_eq!(
            query.effective_statuses(),
            vec![ContactStatus::Active, ContactStatus::Removed]
        );

        let query = ContactQuery {
            statuses: vec![ContactStatus::Archived],
            ..ContactQuery::default()
        };
        assert_eq!(query.effective_statuses(), vec![ContactStatus::Archived]);
    }

    #[test]
    fn every_different_status_is_a_valid_transition() {
        for from in [
            ContactStatus::Active,
            ContactStatus::Removed,
            ContactStatus::Archived,
        ] {
            for to in [
                ContactStatus::Active,
                ContactStatus::Removed,
                ContactStatus::Archived,
            ] {
                assert_eq!(can_transition(from, to), from != to);
            }
        }
    }
}
