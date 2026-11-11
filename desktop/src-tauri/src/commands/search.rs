//! Global search: one term across contacts, campaigns and send batches.

use serde::{Deserialize, Serialize};
use tauri::State;
use weasel_core::campaign::CampaignQuery;
use weasel_core::contact::ContactQuery;
use weasel_core::query::PageRequest;
use weasel_core::send::SendBatchQuery;

use crate::commands::campaigns::CampaignDto;
use crate::commands::contacts::ContactDto;
use crate::commands::dto::search_term;
use crate::commands::send_batches::SendBatchDto;
use crate::error::AppError;
use crate::state::{AppState, Services};

pub fn descriptor() -> weasel_core::FeatureDescriptor {
    weasel_core::FeatureDescriptor {
        id: "search",
        title: "Search",
        commands: &["search_global"],
        events: &[],
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SearchResultsDto {
    pub contacts: Vec<ContactDto>,
    pub campaigns: Vec<CampaignDto>,
    pub send_batches: Vec<SendBatchDto>,
}

#[derive(Debug, Clone, Default, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SearchRequest {
    #[serde(default)]
    pub term: String,
    #[serde(default)]
    pub limit: Option<u32>,
}

#[tauri::command]
#[specta::specta]
pub async fn search_global(
    state: State<'_, AppState>,
    request: SearchRequest,
) -> Result<SearchResultsDto, AppError> {
    global(state.services()?, request).await
}

pub(crate) async fn global(
    services: &Services,
    request: SearchRequest,
) -> Result<SearchResultsDto, AppError> {
    let Some(term) = search_term(Some(request.term))? else {
        return Ok(SearchResultsDto {
            contacts: Vec::new(),
            campaigns: Vec::new(),
            send_batches: Vec::new(),
        });
    };
    let limit = request.limit.unwrap_or(5).clamp(1, 20);
    let page = PageRequest::new(limit, 0);

    let contacts = ContactQuery {
        search: Some(term.clone()),
        page,
        ..ContactQuery::default()
    };
    let campaigns = CampaignQuery {
        search: Some(term.clone()),
        page,
        ..CampaignQuery::default()
    };
    let batches = SendBatchQuery {
        search: Some(term),
        page,
        ..SendBatchQuery::default()
    };

    let (contacts, campaigns, batches) = tokio::try_join!(
        services.contacts.list(&contacts),
        services.campaigns.list(&campaigns),
        services.sends.list(&batches),
    )?;

    Ok(SearchResultsDto {
        contacts: contacts.items.into_iter().map(ContactDto::from).collect(),
        campaigns: campaigns.items.into_iter().map(CampaignDto::from).collect(),
        send_batches: batches.items.into_iter().map(SendBatchDto::from).collect(),
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use weasel_core::enums::CampaignStatus;

    use crate::commands::testing::{harness, seed_campaign, seed_contact};

    use super::*;

    #[tokio::test]
    async fn a_blank_term_returns_nothing() {
        let harness = harness().await;
        seed_contact(&harness, "Ada", "Lovelace", "ada@example.edu").await;
        for term in ["", "   "] {
            let results = global(
                &harness.services,
                SearchRequest {
                    term: term.to_owned(),
                    limit: None,
                },
            )
            .await
            .unwrap();
            assert!(results.contacts.is_empty());
            assert!(results.campaigns.is_empty());
            assert!(results.send_batches.is_empty());
        }
    }

    #[tokio::test]
    async fn one_term_matches_every_kind() {
        let harness = harness().await;
        let contact = seed_contact(&harness, "Needle", "Person", "needle@example.edu").await;
        let campaign = seed_campaign(&harness, "Needle campaign").await;
        harness
            .services
            .campaigns
            .set_status(campaign.id, campaign.version, CampaignStatus::Active)
            .await
            .unwrap();
        harness
            .services
            .sends
            .prepare_send(campaign.id, Some(1))
            .await
            .unwrap();

        let results = global(
            &harness.services,
            SearchRequest {
                term: "needle".to_owned(),
                limit: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(results.contacts.len(), 1);
        assert_eq!(results.contacts[0].id, contact.id);
        assert_eq!(results.campaigns.len(), 1);
        assert_eq!(results.campaigns[0].id, campaign.id);
        assert_eq!(results.send_batches.len(), 1);
    }

    #[tokio::test]
    async fn limit_is_clamped_to_twenty() {
        let harness = harness().await;
        for index in 0..21 {
            seed_contact(
                &harness,
                "Needle",
                &format!("Person {index}"),
                &format!("needle{index}@example.edu"),
            )
            .await;
        }
        let results = global(
            &harness.services,
            SearchRequest {
                term: "needle".to_owned(),
                limit: Some(100),
            },
        )
        .await
        .unwrap();
        assert_eq!(results.contacts.len(), 20);
    }
}
