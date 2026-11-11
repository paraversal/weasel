//! Send-batch history commands. Batches are immutable recipient snapshots.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use weasel_core::FeatureDescriptor;
use weasel_core::send::{SendBatch, SendBatchQuery, SendRecord};

use crate::commands::campaigns::PreparedSendDto;
use crate::commands::dto::{Emitted, PageDto, SendRecordStatusDto, finish, page, search_term};
use crate::error::AppError;
use crate::events::DomainEvent;
use crate::state::{AppState, Services};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SendBatchDto {
    pub id: i64,
    pub campaign_id: i64,
    pub campaign_name: String,
    pub created_at: i64,
    pub recipient_count: u64,
    pub note: String,
    pub copied_at: Option<i64>,
}

impl From<SendBatch> for SendBatchDto {
    fn from(batch: SendBatch) -> Self {
        Self {
            id: batch.id,
            campaign_id: batch.campaign_id,
            campaign_name: batch.campaign_name,
            created_at: batch.created_at,
            recipient_count: batch.recipient_count,
            note: batch.note,
            copied_at: batch.copied_at,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SendRecordDto {
    pub id: i64,
    pub batch_id: i64,
    pub contact_id: Option<i64>,
    pub email: String,
    pub first_name: String,
    pub last_name: String,
    pub status: SendRecordStatusDto,
    pub created_at: i64,
}

impl From<SendRecord> for SendRecordDto {
    fn from(record: SendRecord) -> Self {
        Self {
            id: record.id,
            batch_id: record.batch_id,
            contact_id: record.contact_id,
            email: record.email,
            first_name: record.first_name,
            last_name: record.last_name,
            status: record.status.into(),
            created_at: record.created_at,
        }
    }
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SendBatchListRequest {
    pub campaign_id: Option<i64>,
    pub search: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SendBatchGetRequest {
    pub id: i64,
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SendBatchRecipientsRequest {
    pub id: i64,
    pub search: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SendBatchSetNoteRequest {
    pub id: i64,
    pub note: String,
}

pub fn descriptor() -> FeatureDescriptor {
    FeatureDescriptor {
        id: "send-batches",
        title: "Send history",
        commands: &[
            "send_batches_list",
            "send_batches_get",
            "send_batches_recipients",
            "send_batches_recopy",
            "send_batches_set_note",
        ],
        events: &["sendBatchesChanged"],
    }
}

#[tauri::command]
#[specta::specta]
pub async fn send_batches_list(
    state: State<'_, AppState>,
    request: SendBatchListRequest,
) -> Result<PageDto<SendBatchDto>, AppError> {
    list(state.services()?, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn send_batches_get(
    state: State<'_, AppState>,
    request: SendBatchGetRequest,
) -> Result<SendBatchDto, AppError> {
    get(state.services()?, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn send_batches_recipients(
    state: State<'_, AppState>,
    request: SendBatchRecipientsRequest,
) -> Result<PageDto<SendRecordDto>, AppError> {
    recipients(state.services()?, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn send_batches_recopy(
    app: AppHandle,
    state: State<'_, AppState>,
    request: SendBatchGetRequest,
) -> Result<PreparedSendDto, AppError> {
    let emitted = recopy(state.services()?, request).await?;
    Ok(finish(&app, &state, emitted))
}

#[tauri::command]
#[specta::specta]
pub async fn send_batches_set_note(
    app: AppHandle,
    state: State<'_, AppState>,
    request: SendBatchSetNoteRequest,
) -> Result<SendBatchDto, AppError> {
    let emitted = set_note(state.services()?, request).await?;
    Ok(finish(&app, &state, emitted))
}

pub(crate) async fn list(
    services: &Services,
    request: SendBatchListRequest,
) -> Result<PageDto<SendBatchDto>, AppError> {
    let query = SendBatchQuery {
        campaign_id: request.campaign_id,
        search: search_term(request.search)?,
        page: page(request.limit, request.offset),
    };
    Ok(services.sends.list(&query).await?.into())
}

pub(crate) async fn get(
    services: &Services,
    request: SendBatchGetRequest,
) -> Result<SendBatchDto, AppError> {
    Ok(services.sends.get(request.id).await?.into())
}

pub(crate) async fn recipients(
    services: &Services,
    request: SendBatchRecipientsRequest,
) -> Result<PageDto<SendRecordDto>, AppError> {
    let search = search_term(request.search)?;
    let page = page(request.limit, request.offset);
    let records = services
        .sends
        .recipients(request.id, search.as_deref(), &page)
        .await?;
    Ok(records.into())
}

pub(crate) async fn recopy(
    services: &Services,
    request: SendBatchGetRequest,
) -> Result<Emitted<PreparedSendDto>, AppError> {
    let prepared = services.sends.recopy(request.id).await?;
    let recipient_count = u64::try_from(prepared.emails.len()).unwrap_or(u64::MAX);
    Ok(Emitted::one(
        PreparedSendDto {
            batch_id: prepared.batch.id,
            recipient_count,
            emails: prepared.emails,
        },
        DomainEvent::SendBatchesChanged {},
    ))
}

pub(crate) async fn set_note(
    services: &Services,
    request: SendBatchSetNoteRequest,
) -> Result<Emitted<SendBatchDto>, AppError> {
    let batch = services.sends.set_note(request.id, request.note).await?;
    Ok(Emitted::one(
        batch.into(),
        DomainEvent::SendBatchesChanged {},
    ))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use weasel_core::enums::CampaignStatus;

    use crate::commands::testing::{harness, seed_campaign, seed_contact};

    use super::*;

    #[tokio::test]
    async fn history_commands_read_the_snapshot() {
        let harness = harness().await;
        seed_contact(&harness, "Ada", "Lovelace", "ada@example.com").await;
        seed_contact(&harness, "Bob", "Baker", "bob@example.com").await;
        let campaign = seed_campaign(&harness, "Spring").await;
        let campaign = harness
            .services
            .campaigns
            .set_status(campaign.id, campaign.version, CampaignStatus::Active)
            .await
            .unwrap();
        let prepared = harness
            .services
            .sends
            .prepare_send(campaign.id, None)
            .await
            .unwrap();

        let page = list(
            &harness.services,
            SendBatchListRequest {
                campaign_id: Some(campaign.id),
                search: None,
                limit: None,
                offset: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].id, prepared.batch.id);
        assert_eq!(page.items[0].recipient_count, 2);

        let batch = get(
            &harness.services,
            SendBatchGetRequest {
                id: prepared.batch.id,
            },
        )
        .await
        .unwrap();
        assert_eq!(batch.campaign_name, "Spring");

        let records = recipients(
            &harness.services,
            SendBatchRecipientsRequest {
                id: prepared.batch.id,
                search: Some("Baker".to_owned()),
                limit: None,
                offset: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(records.total, 1);
        assert_eq!(records.items[0].email, "bob@example.com");
    }
}
