//! Contact domain types.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    CoreError, CoreResult,
    email::EmailAddress,
    enums::{ContactSource, ContactStatus, EmailChangeReason},
    identity::{IdentityStrategy, clean_name},
    limits,
    ports::{Clock, ContactRepository},
    query::{Page, PageRequest},
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
    /// New name-derived key, set when either name changed.
    pub identity_key: Option<String>,
}

/// Manual contact edits: validation, normalization, and the no-op shortcut.
pub struct ContactService {
    repo: Arc<dyn ContactRepository>,
    clock: Arc<dyn Clock>,
    identity: Arc<dyn IdentityStrategy>,
}

impl ContactService {
    pub fn new(
        repo: Arc<dyn ContactRepository>,
        clock: Arc<dyn Clock>,
        identity: Arc<dyn IdentityStrategy>,
    ) -> Self {
        Self {
            repo,
            clock,
            identity,
        }
    }

    pub async fn create_manual(&self, input: NewContact) -> CoreResult<Contact> {
        let first_name = cleaned_name("firstName", &input.first_name)?;
        let last_name = cleaned_name("lastName", &input.last_name)?;
        let email = EmailAddress::parse(&input.email)?;
        let notes = checked_notes(&input.notes)?;
        let identity_key = self.identity.key(&first_name, &last_name);
        self.repo
            .create(
                NewContactRecord {
                    first_name,
                    last_name,
                    email,
                    notes,
                    source: ContactSource::Manual,
                    identity_key,
                },
                self.clock.now_ms(),
            )
            .await
    }

    /// Applies only the fields that differ; an empty patch returns the stored row
    /// without writing.
    pub async fn update(&self, id: i64, patch: ContactPatch) -> CoreResult<Contact> {
        let current = self.repo.get(id).await?;

        let first_name = match &patch.first_name {
            Some(raw) => cleaned_name("firstName", raw)?,
            None => current.first_name.clone(),
        };
        let last_name = match &patch.last_name {
            Some(raw) => cleaned_name("lastName", raw)?,
            None => current.last_name.clone(),
        };
        let email = match &patch.email {
            Some(raw) => EmailAddress::parse(raw)?,
            None => EmailAddress {
                display: current.email.clone(),
                normalized: current.email_normalized.clone(),
            },
        };
        let notes = match &patch.notes {
            Some(raw) => checked_notes(raw)?,
            None => current.notes.clone(),
        };

        let mut changes = ContactChanges::default();
        if first_name != current.first_name {
            changes.first_name = Some(first_name);
        }
        if last_name != current.last_name {
            changes.last_name = Some(last_name);
        }
        if email.display != current.email || email.normalized != current.email_normalized {
            changes.email = Some(email);
        }
        if notes != current.notes {
            changes.notes = Some(notes);
        }
        if changes == ContactChanges::default() {
            return Ok(current);
        }
        if changes.first_name.is_some() || changes.last_name.is_some() {
            changes.identity_key = self.identity.key(
                changes.first_name.as_deref().unwrap_or(&current.first_name),
                changes.last_name.as_deref().unwrap_or(&current.last_name),
            );
        }

        self.repo
            .update(
                id,
                patch.version,
                changes,
                EmailChangeReason::Manual,
                self.clock.now_ms(),
            )
            .await
    }

    pub async fn set_status(
        &self,
        id: i64,
        expected_version: i64,
        status: ContactStatus,
    ) -> CoreResult<Contact> {
        self.repo
            .set_status(id, expected_version, status, self.clock.now_ms())
            .await
    }

    pub async fn delete(&self, id: i64, expected_version: i64) -> CoreResult<()> {
        self.repo
            .delete(id, expected_version, self.clock.now_ms())
            .await
    }

    pub async fn get(&self, id: i64) -> CoreResult<Contact> {
        self.repo.get(id).await
    }

    pub async fn detail(&self, id: i64) -> CoreResult<ContactDetail> {
        self.repo.detail(id).await
    }

    pub async fn list(&self, query: &ContactQuery) -> CoreResult<Page<Contact>> {
        self.repo.list(query).await
    }

    pub async fn stats(&self) -> CoreResult<ContactStats> {
        self.repo.stats().await
    }

    pub async fn count_active(&self) -> CoreResult<u64> {
        self.repo.count_active().await
    }
}

fn cleaned_name(field: &'static str, raw: &str) -> CoreResult<String> {
    let name = clean_name(raw);
    if name.is_empty() {
        return Err(CoreError::validation(field, "is required"));
    }
    if name.chars().count() > limits::NAME_MAX {
        return Err(CoreError::validation(
            field,
            format!("must be at most {} characters", limits::NAME_MAX),
        ));
    }
    Ok(name)
}

fn checked_notes(raw: &str) -> CoreResult<String> {
    if raw.chars().count() > limits::NOTES_MAX {
        return Err(CoreError::validation(
            "notes",
            format!("must be at most {} characters", limits::NOTES_MAX),
        ));
    }
    Ok(raw.to_owned())
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
