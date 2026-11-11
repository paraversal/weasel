//! Campaign domain types and status transitions.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::{
    CoreError, CoreResult,
    enums::{CampaignStatus, ContactStatus},
    limits,
    ports::{CampaignRepository, Clock},
    query::{Page, PageRequest},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Campaign {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub subject: String,
    pub body: String,
    pub status: CampaignStatus,
    pub created_at: i64,
    pub updated_at: i64,
    pub version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewCampaign {
    pub name: String,
    pub description: String,
    pub subject: String,
    pub body: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CampaignPatch {
    pub version: i64,
    pub name: Option<String>,
    pub description: Option<String>,
    pub subject: Option<String>,
    pub body: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CampaignQuery {
    pub status: Option<CampaignStatus>,
    pub search: Option<String>,
    pub page: PageRequest,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CampaignStats {
    /// Delivered send records for this campaign (bounces excluded), including those
    /// whose contact was deleted.
    pub sent: u64,
    /// Active contacts with no delivered record whose address is not on hold: the next
    /// send's recipients.
    pub unsent: u64,
    /// Active contacts with no delivered record whose address is on hold.
    pub on_hold: u64,
    /// Send records marked as bounced.
    pub bounced: u64,
    /// Removed contacts with no delivered record.
    pub removed: u64,
    /// `sent + unsent + on_hold`.
    pub total: u64,
    pub last_batch_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CampaignSummary {
    pub campaign: Campaign,
    pub stats: CampaignStats,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AudienceFilter {
    Sent,
    Unsent,
    All,
}

/// A row in the send preview. Sent rows carry the snapshot, not current contact data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudienceMember {
    pub contact_id: Option<i64>,
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub contact_status: Option<ContactStatus>,
    pub batch_id: Option<i64>,
    pub sent_at: Option<i64>,
    /// Set on sent rows.
    pub record_id: Option<i64>,
    /// A sent row whose record was marked as bounced.
    pub bounced: bool,
    /// An unsent row whose current address is on hold.
    pub on_hold: bool,
}

/// Draft → active or archived; active → archived; archived → active.
pub fn can_transition(from: CampaignStatus, to: CampaignStatus) -> bool {
    use CampaignStatus::{Active, Archived, Draft};
    matches!(
        (from, to),
        (Draft, Active) | (Draft, Archived) | (Active, Archived) | (Archived, Active)
    )
}

/// Manual campaign edits: validation, transitions, and the no-op shortcut.
pub struct CampaignService {
    repo: Arc<dyn CampaignRepository>,
    clock: Arc<dyn Clock>,
}

impl CampaignService {
    pub fn new(repo: Arc<dyn CampaignRepository>, clock: Arc<dyn Clock>) -> Self {
        Self { repo, clock }
    }

    pub async fn create(&self, input: NewCampaign) -> CoreResult<Campaign> {
        let campaign = NewCampaign {
            name: cleaned_campaign_name(&input.name)?,
            description: checked_text("description", &input.description, limits::DESCRIPTION_MAX)?,
            subject: checked_text("subject", &input.subject, limits::SUBJECT_MAX)?,
            body: checked_text("body", &input.body, limits::BODY_MAX)?,
        };
        self.repo.create(campaign, self.clock.now_ms()).await
    }

    /// Applies only the fields that differ; an empty patch returns the stored row
    /// without writing.
    pub async fn update(&self, id: i64, patch: CampaignPatch) -> CoreResult<Campaign> {
        let current = self.repo.get(id).await?;
        let version = patch.version;

        let name = match &patch.name {
            Some(raw) => cleaned_campaign_name(raw)?,
            None => current.name.clone(),
        };
        let description = match &patch.description {
            Some(raw) => checked_text("description", raw, limits::DESCRIPTION_MAX)?,
            None => current.description.clone(),
        };
        let subject = match &patch.subject {
            Some(raw) => checked_text("subject", raw, limits::SUBJECT_MAX)?,
            None => current.subject.clone(),
        };
        let body = match &patch.body {
            Some(raw) => checked_text("body", raw, limits::BODY_MAX)?,
            None => current.body.clone(),
        };

        let mut changes = CampaignPatch {
            version,
            ..CampaignPatch::default()
        };
        if name != current.name {
            changes.name = Some(name);
        }
        if description != current.description {
            changes.description = Some(description);
        }
        if subject != current.subject {
            changes.subject = Some(subject);
        }
        if body != current.body {
            changes.body = Some(body);
        }
        if changes.name.is_none()
            && changes.description.is_none()
            && changes.subject.is_none()
            && changes.body.is_none()
        {
            return Ok(current);
        }

        self.repo.update(id, changes, self.clock.now_ms()).await
    }

    pub async fn set_status(
        &self,
        id: i64,
        expected_version: i64,
        status: CampaignStatus,
    ) -> CoreResult<Campaign> {
        let current = self.repo.get(id).await?;
        if !can_transition(current.status, status) {
            return Err(CoreError::InvalidState(format!(
                "A campaign cannot move from {} to {}.",
                current.status.as_str(),
                status.as_str()
            )));
        }
        self.repo
            .set_status(id, expected_version, status, self.clock.now_ms())
            .await
    }

    pub async fn delete(&self, id: i64, expected_version: i64) -> CoreResult<()> {
        self.repo
            .delete(id, expected_version, self.clock.now_ms())
            .await
    }

    pub async fn get(&self, id: i64) -> CoreResult<Campaign> {
        self.repo.get(id).await
    }

    pub async fn list(&self, query: &CampaignQuery) -> CoreResult<Page<Campaign>> {
        self.repo.list(query).await
    }

    /// Campaigns with their stats, using one batched stats lookup for the whole page.
    pub async fn list_summaries(&self, query: &CampaignQuery) -> CoreResult<Page<CampaignSummary>> {
        let page = self.repo.list(query).await?;
        let ids: Vec<i64> = page.items.iter().map(|campaign| campaign.id).collect();
        let mut stats = self.repo.stats_for(&ids).await?;
        Ok(page.map(|campaign| {
            let stats = stats.remove(&campaign.id).unwrap_or_default();
            CampaignSummary { campaign, stats }
        }))
    }

    pub async fn stats(&self, id: i64) -> CoreResult<CampaignStats> {
        self.repo.stats(id).await
    }

    pub async fn audience(
        &self,
        id: i64,
        filter: AudienceFilter,
        search: Option<&str>,
        page: &PageRequest,
    ) -> CoreResult<Page<AudienceMember>> {
        self.repo.audience(id, filter, search, page).await
    }
}

fn cleaned_campaign_name(raw: &str) -> CoreResult<String> {
    let name = raw.trim();
    if name.is_empty() {
        return Err(CoreError::validation("name", "is required"));
    }
    if name.chars().count() > limits::CAMPAIGN_NAME_MAX {
        return Err(CoreError::validation(
            "name",
            format!("is longer than {} characters", limits::CAMPAIGN_NAME_MAX),
        ));
    }
    Ok(name.to_owned())
}

fn checked_text(field: &'static str, raw: &str, max: usize) -> CoreResult<String> {
    if raw.chars().count() > max {
        return Err(CoreError::validation(
            field,
            format!("is longer than {max} characters"),
        ));
    }
    Ok(raw.to_owned())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn campaign_transition_table() {
        use CampaignStatus::{Active, Archived, Draft};
        let allowed = [
            (Draft, Active),
            (Draft, Archived),
            (Active, Archived),
            (Archived, Active),
        ];
        for from in [Draft, Active, Archived] {
            for to in [Draft, Active, Archived] {
                assert_eq!(
                    can_transition(from, to),
                    allowed.contains(&(from, to)),
                    "{from:?} -> {to:?}"
                );
            }
        }
    }
}
