//! Campaign commands: CRUD, stats, audience preview and send preparation.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use weasel_core::FeatureDescriptor;
use weasel_core::campaign::{
    AudienceFilter, AudienceMember, Campaign, CampaignPatch, CampaignQuery, CampaignStats,
    CampaignSummary, NewCampaign,
};

use crate::commands::dto::{
    AudienceFilterDto, CampaignStatusDto, ContactStatusDto, Emitted, PageDto, finish, page,
    search_term,
};
use crate::error::AppError;
use crate::events::DomainEvent;
use crate::state::{AppState, Services};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CampaignDto {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub subject: String,
    pub body: String,
    pub status: CampaignStatusDto,
    pub created_at: i64,
    pub updated_at: i64,
    pub version: i64,
}

impl From<Campaign> for CampaignDto {
    fn from(campaign: Campaign) -> Self {
        Self {
            id: campaign.id,
            name: campaign.name,
            description: campaign.description,
            subject: campaign.subject,
            body: campaign.body,
            status: campaign.status.into(),
            created_at: campaign.created_at,
            updated_at: campaign.updated_at,
            version: campaign.version,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CampaignStatsDto {
    pub sent: u64,
    pub unsent: u64,
    pub removed: u64,
    pub total: u64,
    pub last_batch_at: Option<i64>,
}

impl From<CampaignStats> for CampaignStatsDto {
    fn from(stats: CampaignStats) -> Self {
        Self {
            sent: stats.sent,
            unsent: stats.unsent,
            removed: stats.removed,
            total: stats.total,
            last_batch_at: stats.last_batch_at,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CampaignSummaryDto {
    pub campaign: CampaignDto,
    pub stats: CampaignStatsDto,
}

impl From<CampaignSummary> for CampaignSummaryDto {
    fn from(summary: CampaignSummary) -> Self {
        Self {
            campaign: summary.campaign.into(),
            stats: summary.stats.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AudienceMemberDto {
    pub contact_id: Option<i64>,
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub contact_status: Option<ContactStatusDto>,
    pub batch_id: Option<i64>,
    pub sent_at: Option<i64>,
}

impl From<AudienceMember> for AudienceMemberDto {
    fn from(member: AudienceMember) -> Self {
        Self {
            contact_id: member.contact_id,
            first_name: member.first_name,
            last_name: member.last_name,
            email: member.email,
            contact_status: member.contact_status.map(Into::into),
            batch_id: member.batch_id,
            sent_at: member.sent_at,
        }
    }
}

/// The addresses copied for a send, so the UI can show and re-copy them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PreparedSendDto {
    pub batch_id: i64,
    pub recipient_count: u64,
    pub emails: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CampaignListRequest {
    pub status: Option<CampaignStatusDto>,
    pub search: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CampaignGetRequest {
    pub id: i64,
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CampaignCreateRequest {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub subject: Option<String>,
    #[serde(default)]
    pub body: Option<String>,
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CampaignUpdateRequest {
    pub id: i64,
    pub version: i64,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub subject: Option<String>,
    #[serde(default)]
    pub body: Option<String>,
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CampaignStatusRequest {
    pub id: i64,
    pub version: i64,
    pub status: CampaignStatusDto,
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CampaignDeleteRequest {
    pub id: i64,
    pub version: i64,
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CampaignStatsRequest {
    pub id: i64,
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CampaignAudienceRequest {
    pub campaign_id: i64,
    pub filter: AudienceFilterDto,
    pub search: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CampaignPrepareSendRequest {
    pub campaign_id: i64,
    pub expected_count: Option<u64>,
}

pub fn descriptor() -> FeatureDescriptor {
    FeatureDescriptor {
        id: "campaigns",
        title: "Campaigns",
        commands: &[
            "campaigns_list",
            "campaigns_get",
            "campaigns_create",
            "campaigns_update",
            "campaigns_set_status",
            "campaigns_delete",
            "campaigns_stats",
            "campaigns_audience",
            "campaigns_prepare_send",
        ],
        events: &["campaignsChanged", "sendBatchesChanged"],
    }
}

#[tauri::command]
#[specta::specta]
pub async fn campaigns_list(
    state: State<'_, AppState>,
    request: CampaignListRequest,
) -> Result<PageDto<CampaignSummaryDto>, AppError> {
    list(state.services()?, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn campaigns_get(
    state: State<'_, AppState>,
    request: CampaignGetRequest,
) -> Result<CampaignDto, AppError> {
    get(state.services()?, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn campaigns_create(
    app: AppHandle,
    state: State<'_, AppState>,
    request: CampaignCreateRequest,
) -> Result<CampaignDto, AppError> {
    let emitted = create(state.services()?, request).await?;
    Ok(finish(&app, &state, emitted))
}

#[tauri::command]
#[specta::specta]
pub async fn campaigns_update(
    app: AppHandle,
    state: State<'_, AppState>,
    request: CampaignUpdateRequest,
) -> Result<CampaignDto, AppError> {
    let emitted = update(state.services()?, request).await?;
    Ok(finish(&app, &state, emitted))
}

#[tauri::command]
#[specta::specta]
pub async fn campaigns_set_status(
    app: AppHandle,
    state: State<'_, AppState>,
    request: CampaignStatusRequest,
) -> Result<CampaignDto, AppError> {
    let emitted = set_status(state.services()?, request).await?;
    Ok(finish(&app, &state, emitted))
}

#[tauri::command]
#[specta::specta]
pub async fn campaigns_delete(
    app: AppHandle,
    state: State<'_, AppState>,
    request: CampaignDeleteRequest,
) -> Result<(), AppError> {
    let emitted = delete(state.services()?, request).await?;
    finish(&app, &state, emitted);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn campaigns_stats(
    state: State<'_, AppState>,
    request: CampaignStatsRequest,
) -> Result<CampaignStatsDto, AppError> {
    stats(state.services()?, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn campaigns_audience(
    state: State<'_, AppState>,
    request: CampaignAudienceRequest,
) -> Result<PageDto<AudienceMemberDto>, AppError> {
    audience(state.services()?, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn campaigns_prepare_send(
    app: AppHandle,
    state: State<'_, AppState>,
    request: CampaignPrepareSendRequest,
) -> Result<PreparedSendDto, AppError> {
    let emitted = prepare_send(state.services()?, request).await?;
    Ok(finish(&app, &state, emitted))
}

pub(crate) async fn list(
    services: &Services,
    request: CampaignListRequest,
) -> Result<PageDto<CampaignSummaryDto>, AppError> {
    let query = CampaignQuery {
        status: request.status.map(Into::into),
        search: search_term(request.search)?,
        page: page(request.limit, request.offset),
    };
    Ok(services.campaigns.list_summaries(&query).await?.into())
}

pub(crate) async fn get(
    services: &Services,
    request: CampaignGetRequest,
) -> Result<CampaignDto, AppError> {
    Ok(services.campaigns.get(request.id).await?.into())
}

pub(crate) async fn create(
    services: &Services,
    request: CampaignCreateRequest,
) -> Result<Emitted<CampaignDto>, AppError> {
    let campaign = services
        .campaigns
        .create(NewCampaign {
            name: request.name,
            description: request.description.unwrap_or_default(),
            subject: request.subject.unwrap_or_default(),
            body: request.body.unwrap_or_default(),
        })
        .await?;
    Ok(Emitted::one(
        campaign.into(),
        DomainEvent::CampaignsChanged {},
    ))
}

pub(crate) async fn update(
    services: &Services,
    request: CampaignUpdateRequest,
) -> Result<Emitted<CampaignDto>, AppError> {
    let campaign = services
        .campaigns
        .update(
            request.id,
            CampaignPatch {
                version: request.version,
                name: request.name,
                description: request.description,
                subject: request.subject,
                body: request.body,
            },
        )
        .await?;
    Ok(Emitted::one(
        campaign.into(),
        DomainEvent::CampaignsChanged {},
    ))
}

pub(crate) async fn set_status(
    services: &Services,
    request: CampaignStatusRequest,
) -> Result<Emitted<CampaignDto>, AppError> {
    let campaign = services
        .campaigns
        .set_status(request.id, request.version, request.status.into())
        .await?;
    Ok(Emitted::one(
        campaign.into(),
        DomainEvent::CampaignsChanged {},
    ))
}

pub(crate) async fn delete(
    services: &Services,
    request: CampaignDeleteRequest,
) -> Result<Emitted<()>, AppError> {
    services
        .campaigns
        .delete(request.id, request.version)
        .await?;
    Ok(Emitted::one((), DomainEvent::CampaignsChanged {}))
}

pub(crate) async fn stats(
    services: &Services,
    request: CampaignStatsRequest,
) -> Result<CampaignStatsDto, AppError> {
    Ok(services.campaigns.stats(request.id).await?.into())
}

pub(crate) async fn audience(
    services: &Services,
    request: CampaignAudienceRequest,
) -> Result<PageDto<AudienceMemberDto>, AppError> {
    let search = search_term(request.search)?;
    let page = page(request.limit, request.offset);
    let members = services
        .campaigns
        .audience(
            request.campaign_id,
            AudienceFilter::from(request.filter),
            search.as_deref(),
            &page,
        )
        .await?;
    Ok(PageDto {
        items: members.items.into_iter().map(Into::into).collect(),
        total: i64::try_from(members.total).unwrap_or(i64::MAX),
        offset: i64::from(members.offset),
        limit: i64::from(members.limit),
    })
}

pub(crate) async fn prepare_send(
    services: &Services,
    request: CampaignPrepareSendRequest,
) -> Result<Emitted<PreparedSendDto>, AppError> {
    let prepared = services
        .sends
        .prepare_send(request.campaign_id, request.expected_count)
        .await?;
    let recipient_count = u64::try_from(prepared.emails.len()).unwrap_or(u64::MAX);
    Ok(Emitted::new(
        PreparedSendDto {
            batch_id: prepared.batch.id,
            recipient_count,
            emails: prepared.emails,
        },
        vec![
            DomainEvent::SendBatchesChanged {},
            DomainEvent::CampaignsChanged {},
        ],
    ))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use weasel_core::campaign::Campaign;
    use weasel_core::contact::ContactPatch;
    use weasel_core::enums::CampaignStatus;
    use weasel_core::limits::BATCH_NOTE_MAX;
    use weasel_core::send::format_recipients;

    use crate::commands::testing::{harness, seed_campaign, seed_contact};
    use crate::error::ErrorCode;

    use super::*;

    fn list_request() -> CampaignListRequest {
        CampaignListRequest {
            status: None,
            search: None,
            limit: None,
            offset: None,
        }
    }

    fn prepare_request(campaign_id: i64, expected: Option<u64>) -> CampaignPrepareSendRequest {
        CampaignPrepareSendRequest {
            campaign_id,
            expected_count: expected,
        }
    }

    async fn active(harness: &crate::commands::testing::Harness, name: &str) -> Campaign {
        let campaign = seed_campaign(harness, name).await;
        harness
            .services
            .campaigns
            .set_status(campaign.id, campaign.version, CampaignStatus::Active)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn list_returns_stats_for_every_row() {
        let harness = harness().await;
        seed_contact(&harness, "Ada", "Lovelace", "ada@example.com").await;
        let used = active(&harness, "Used").await;
        let idle = active(&harness, "Idle").await;
        prepare_send(&harness.services, prepare_request(used.id, None))
            .await
            .unwrap();

        let page = list(&harness.services, list_request()).await.unwrap();
        assert_eq!(page.total, 2);
        let used_row = page
            .items
            .iter()
            .find(|row| row.campaign.id == used.id)
            .unwrap();
        assert_eq!(used_row.stats.sent, 1);
        assert_eq!(used_row.stats.unsent, 0);
        let idle_row = page
            .items
            .iter()
            .find(|row| row.campaign.id == idle.id)
            .unwrap();
        assert_eq!(idle_row.stats.sent, 0);
        assert_eq!(idle_row.stats.unsent, 1);
    }

    #[tokio::test]
    async fn prepare_with_the_reviewed_count_copies_and_emits() {
        let harness = harness().await;
        seed_contact(&harness, "Ada", "Lovelace", "ada@example.com").await;
        seed_contact(&harness, "Bob", "Baker", "bob@example.com").await;
        let campaign = active(&harness, "Spring").await;

        let prepared = prepare_send(&harness.services, prepare_request(campaign.id, Some(2)))
            .await
            .unwrap();
        assert_eq!(prepared.value.recipient_count, 2);
        assert_eq!(
            harness.clipboard.writes().last().unwrap(),
            &format_recipients(&prepared.value.emails)
        );
        assert_eq!(
            prepared.events,
            vec![
                DomainEvent::SendBatchesChanged {},
                DomainEvent::CampaignsChanged {},
            ]
        );
    }

    #[tokio::test]
    async fn wrong_reviewed_count_leaves_no_batch() {
        let harness = harness().await;
        seed_contact(&harness, "Ada", "Lovelace", "ada@example.com").await;
        let campaign = active(&harness, "Spring").await;

        let err = prepare_send(&harness.services, prepare_request(campaign.id, Some(9)))
            .await
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::Conflict);
        assert_eq!(
            err.message,
            "The audience changed since you reviewed it. Review it again."
        );
        assert!(harness.clipboard.writes().is_empty());
        assert_eq!(
            harness
                .services
                .sends
                .list(&weasel_core::send::SendBatchQuery::default())
                .await
                .unwrap()
                .total,
            0
        );
    }

    #[tokio::test]
    async fn clipboard_failure_is_retryable_and_emits_nothing() {
        let harness = harness().await;
        seed_contact(&harness, "Ada", "Lovelace", "ada@example.com").await;
        let campaign = active(&harness, "Spring").await;

        harness.clipboard.fail_next();
        let err = prepare_send(&harness.services, prepare_request(campaign.id, None))
            .await
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::Io);
        assert!(err.retryable);
        assert_eq!(
            harness
                .services
                .sends
                .list(&weasel_core::send::SendBatchQuery::default())
                .await
                .unwrap()
                .total,
            0
        );
    }

    #[tokio::test]
    async fn draft_campaign_cannot_send() {
        let harness = harness().await;
        seed_contact(&harness, "Ada", "Lovelace", "ada@example.com").await;
        let draft = seed_campaign(&harness, "Draft").await;

        let err = prepare_send(&harness.services, prepare_request(draft.id, None))
            .await
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidState);
    }

    #[tokio::test]
    async fn recopy_uses_the_original_snapshot() {
        let harness = harness().await;
        let ada = seed_contact(&harness, "Ada", "Lovelace", "ada@example.com").await;
        let campaign = active(&harness, "Spring").await;
        let prepared = prepare_send(&harness.services, prepare_request(campaign.id, None))
            .await
            .unwrap();

        harness
            .services
            .contacts
            .update(
                ada.id,
                ContactPatch {
                    version: ada.version,
                    email: Some("ada@new.example.com".to_owned()),
                    ..ContactPatch::default()
                },
            )
            .await
            .unwrap();

        let recopied = crate::commands::send_batches::recopy(
            &harness.services,
            crate::commands::send_batches::SendBatchGetRequest {
                id: prepared.value.batch_id,
            },
        )
        .await
        .unwrap();
        assert_eq!(recopied.value.emails, prepared.value.emails);
        assert_eq!(
            harness.clipboard.writes().last().unwrap(),
            &format_recipients(&recopied.value.emails)
        );
        assert_eq!(recopied.events, vec![DomainEvent::SendBatchesChanged {}]);
    }

    #[tokio::test]
    async fn note_length_is_validated() {
        let harness = harness().await;
        seed_contact(&harness, "Ada", "Lovelace", "ada@example.com").await;
        let campaign = active(&harness, "Spring").await;
        let prepared = prepare_send(&harness.services, prepare_request(campaign.id, None))
            .await
            .unwrap();

        let err = crate::commands::send_batches::set_note(
            &harness.services,
            crate::commands::send_batches::SendBatchSetNoteRequest {
                id: prepared.value.batch_id,
                note: "x".repeat(BATCH_NOTE_MAX + 1),
            },
        )
        .await
        .unwrap_err();
        assert_eq!(err.code, ErrorCode::Validation);
    }

    #[tokio::test]
    async fn audience_sent_uses_the_snapshot() {
        let harness = harness().await;
        let ada = seed_contact(&harness, "Ada", "Lovelace", "ada@example.com").await;
        let campaign = active(&harness, "Spring").await;
        prepare_send(&harness.services, prepare_request(campaign.id, None))
            .await
            .unwrap();
        harness
            .services
            .contacts
            .update(
                ada.id,
                ContactPatch {
                    version: ada.version,
                    email: Some("ada@new.example.com".to_owned()),
                    ..ContactPatch::default()
                },
            )
            .await
            .unwrap();

        let page = audience(
            &harness.services,
            CampaignAudienceRequest {
                campaign_id: campaign.id,
                filter: AudienceFilterDto::Sent,
                search: None,
                limit: None,
                offset: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].contact_id, Some(ada.id));
        assert_eq!(page.items[0].email, "ada@example.com");
        assert_eq!(page.items[0].contact_status, Some(ContactStatusDto::Active));
    }

    #[tokio::test]
    async fn deleting_an_empty_campaign_emits_once() {
        let harness = harness().await;
        let campaign = seed_campaign(&harness, "Empty").await;
        let deleted = delete(
            &harness.services,
            CampaignDeleteRequest {
                id: campaign.id,
                version: campaign.version,
            },
        )
        .await
        .unwrap();
        assert_eq!(deleted.events, vec![DomainEvent::CampaignsChanged {}]);
        assert_eq!(
            get(&harness.services, CampaignGetRequest { id: campaign.id })
                .await
                .unwrap_err()
                .code,
            ErrorCode::NotFound
        );
    }

    #[tokio::test]
    async fn update_and_status_are_audited_changes() {
        let harness = harness().await;
        let campaign = seed_campaign(&harness, "Spring").await;
        let updated = update(
            &harness.services,
            CampaignUpdateRequest {
                id: campaign.id,
                version: campaign.version,
                name: Some("Summer".to_owned()),
                description: None,
                subject: None,
                body: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(updated.value.version, campaign.version + 1);
        assert_eq!(updated.events, vec![DomainEvent::CampaignsChanged {}]);
        let moved = set_status(
            &harness.services,
            CampaignStatusRequest {
                id: campaign.id,
                version: updated.value.version,
                status: CampaignStatusDto::Active,
            },
        )
        .await
        .unwrap();
        assert_eq!(moved.value.status, CampaignStatusDto::Active);
        assert_eq!(moved.events, vec![DomainEvent::CampaignsChanged {}]);
        assert_eq!(
            stats(&harness.services, CampaignStatsRequest { id: campaign.id })
                .await
                .unwrap()
                .total,
            0
        );
    }
}
