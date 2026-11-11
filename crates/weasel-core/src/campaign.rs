//! Campaign domain types and status transitions.

use serde::{Deserialize, Serialize};

use crate::{
    enums::{CampaignStatus, ContactStatus},
    query::PageRequest,
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
    /// Send records for this campaign, including those whose contact was deleted.
    pub sent: u64,
    /// Active contacts with no send record.
    pub unsent: u64,
    /// Removed contacts with no send record.
    pub removed: u64,
    /// `sent + unsent`.
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
}

/// Draft → active or archived; active → archived; archived → active.
pub fn can_transition(from: CampaignStatus, to: CampaignStatus) -> bool {
    use CampaignStatus::{Active, Archived, Draft};
    matches!(
        (from, to),
        (Draft, Active) | (Draft, Archived) | (Active, Archived) | (Archived, Active)
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn campaign_transition_table() {
        use CampaignStatus::{Active, Archived, Draft};
        let allowed = [(Draft, Active), (Draft, Archived), (Active, Archived), (Archived, Active)];
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
