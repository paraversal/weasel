//! Contact commands and wire DTOs.
//!
//! Personal data stays behind these types: `identity_key` and the normalized email are
//! storage details, so neither crosses the IPC boundary.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use weasel_core::FeatureDescriptor;
use weasel_core::contact::{
    BatchCount, CampaignReceipt, Contact, ContactDetail, ContactPatch, ContactQuery, ContactSort,
    ContactStats, EmailHistoryEntry, NewContact,
};

use crate::commands::dto::{
    ContactSortDto, ContactSourceDto, ContactStatusDto, EmailChangeReasonDto, Emitted, PageDto,
    finish, page, search_term,
};
use crate::error::AppError;
use crate::events::DomainEvent;
use crate::state::{AppState, Services};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ContactDto {
    pub id: i64,
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub status: ContactStatusDto,
    pub source: ContactSourceDto,
    pub import_batch_id: Option<i64>,
    pub date_added: i64,
    pub updated_at: i64,
    pub version: i64,
    pub notes: String,
}

impl From<Contact> for ContactDto {
    fn from(contact: Contact) -> Self {
        Self {
            id: contact.id,
            first_name: contact.first_name,
            last_name: contact.last_name,
            email: contact.email,
            status: contact.status.into(),
            source: contact.source.into(),
            import_batch_id: contact.import_batch_id,
            date_added: contact.date_added,
            updated_at: contact.updated_at,
            version: contact.version,
            notes: contact.notes,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct EmailHistoryDto {
    pub email: String,
    pub valid_from: i64,
    pub valid_to: Option<i64>,
    pub reason: EmailChangeReasonDto,
}

impl From<EmailHistoryEntry> for EmailHistoryDto {
    fn from(entry: EmailHistoryEntry) -> Self {
        Self {
            email: entry.email,
            valid_from: entry.valid_from,
            valid_to: entry.valid_to,
            reason: entry.reason.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CampaignReceiptDto {
    pub campaign_id: i64,
    pub campaign_name: String,
    pub batch_id: i64,
    pub sent_at: i64,
}

impl From<CampaignReceipt> for CampaignReceiptDto {
    fn from(receipt: CampaignReceipt) -> Self {
        Self {
            campaign_id: receipt.campaign_id,
            campaign_name: receipt.campaign_name,
            batch_id: receipt.batch_id,
            sent_at: receipt.sent_at,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ContactDetailDto {
    pub contact: ContactDto,
    pub email_history: Vec<EmailHistoryDto>,
    pub campaigns: Vec<CampaignReceiptDto>,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct BatchCountDto {
    pub batch_id: i64,
    pub source_name: String,
    pub count: u64,
}

impl From<BatchCount> for BatchCountDto {
    fn from(count: BatchCount) -> Self {
        Self {
            batch_id: count.batch_id,
            source_name: count.source_name,
            count: count.count,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ContactStatsDto {
    pub active: u64,
    pub removed: u64,
    pub archived: u64,
    pub total: u64,
    pub by_batch: Vec<BatchCountDto>,
}

impl From<ContactStats> for ContactStatsDto {
    fn from(stats: ContactStats) -> Self {
        Self {
            active: stats.active,
            removed: stats.removed,
            archived: stats.archived,
            total: stats.total,
            by_batch: stats.by_batch.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ContactListRequest {
    pub search: Option<String>,
    pub statuses: Option<Vec<ContactStatusDto>>,
    pub import_batch_id: Option<i64>,
    pub sort: Option<ContactSortDto>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ContactGetRequest {
    pub id: i64,
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ContactCreateRequest {
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ContactUpdateRequest {
    pub id: i64,
    pub version: i64,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub email: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ContactStatusRequest {
    pub id: i64,
    pub version: i64,
    pub status: ContactStatusDto,
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ContactDeleteRequest {
    pub id: i64,
    pub version: i64,
}

pub fn descriptor() -> FeatureDescriptor {
    FeatureDescriptor {
        id: "contacts",
        title: "Contacts",
        commands: &[
            "contacts_list",
            "contacts_get",
            "contacts_create",
            "contacts_update",
            "contacts_set_status",
            "contacts_delete",
            "contacts_stats",
        ],
        events: &["contactsChanged"],
    }
}

#[tauri::command]
#[specta::specta]
pub async fn contacts_list(
    state: State<'_, AppState>,
    request: ContactListRequest,
) -> Result<PageDto<ContactDto>, AppError> {
    list(state.services()?, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn contacts_get(
    state: State<'_, AppState>,
    request: ContactGetRequest,
) -> Result<ContactDetailDto, AppError> {
    get(state.services()?, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn contacts_create(
    app: AppHandle,
    state: State<'_, AppState>,
    request: ContactCreateRequest,
) -> Result<ContactDto, AppError> {
    let emitted = create(state.services()?, request).await?;
    Ok(finish(&app, &state, emitted))
}

#[tauri::command]
#[specta::specta]
pub async fn contacts_update(
    app: AppHandle,
    state: State<'_, AppState>,
    request: ContactUpdateRequest,
) -> Result<ContactDto, AppError> {
    let emitted = update(state.services()?, request).await?;
    Ok(finish(&app, &state, emitted))
}

#[tauri::command]
#[specta::specta]
pub async fn contacts_set_status(
    app: AppHandle,
    state: State<'_, AppState>,
    request: ContactStatusRequest,
) -> Result<ContactDto, AppError> {
    let emitted = set_status(state.services()?, request).await?;
    Ok(finish(&app, &state, emitted))
}

#[tauri::command]
#[specta::specta]
pub async fn contacts_delete(
    app: AppHandle,
    state: State<'_, AppState>,
    request: ContactDeleteRequest,
) -> Result<(), AppError> {
    let emitted = delete(state.services()?, request).await?;
    finish(&app, &state, emitted);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn contacts_stats(state: State<'_, AppState>) -> Result<ContactStatsDto, AppError> {
    stats(state.services()?).await
}

pub(crate) async fn list(
    services: &Services,
    request: ContactListRequest,
) -> Result<PageDto<ContactDto>, AppError> {
    let query = ContactQuery {
        search: search_term(request.search)?,
        statuses: request
            .statuses
            .unwrap_or_default()
            .into_iter()
            .map(Into::into)
            .collect(),
        import_batch_id: request.import_batch_id,
        sort: request
            .sort
            .map(Into::into)
            .unwrap_or(ContactSort::LastNameAsc),
        page: page(request.limit, request.offset),
    };
    Ok(services.contacts.list(&query).await?.into())
}

pub(crate) async fn get(
    services: &Services,
    request: ContactGetRequest,
) -> Result<ContactDetailDto, AppError> {
    let detail = services.contacts.detail(request.id).await?;
    Ok(detail.into())
}

pub(crate) async fn create(
    services: &Services,
    request: ContactCreateRequest,
) -> Result<Emitted<ContactDto>, AppError> {
    let contact = services
        .contacts
        .create_manual(NewContact {
            first_name: request.first_name,
            last_name: request.last_name,
            email: request.email,
            notes: request.notes.unwrap_or_default(),
        })
        .await?;
    Ok(Emitted::one(
        contact.into(),
        DomainEvent::ContactsChanged {},
    ))
}

pub(crate) async fn update(
    services: &Services,
    request: ContactUpdateRequest,
) -> Result<Emitted<ContactDto>, AppError> {
    let contact = services
        .contacts
        .update(
            request.id,
            ContactPatch {
                version: request.version,
                first_name: request.first_name,
                last_name: request.last_name,
                email: request.email,
                notes: request.notes,
            },
        )
        .await?;
    let changed = contact.version != request.version;
    let dto = contact.into();
    Ok(if changed {
        Emitted::one(dto, DomainEvent::ContactsChanged {})
    } else {
        Emitted::read(dto)
    })
}

pub(crate) async fn set_status(
    services: &Services,
    request: ContactStatusRequest,
) -> Result<Emitted<ContactDto>, AppError> {
    let contact = services
        .contacts
        .set_status(request.id, request.version, request.status.into())
        .await?;
    let changed = contact.version != request.version;
    let dto = contact.into();
    Ok(if changed {
        Emitted::one(dto, DomainEvent::ContactsChanged {})
    } else {
        Emitted::read(dto)
    })
}

pub(crate) async fn delete(
    services: &Services,
    request: ContactDeleteRequest,
) -> Result<Emitted<()>, AppError> {
    services
        .contacts
        .delete(request.id, request.version)
        .await?;
    Ok(Emitted::one((), DomainEvent::ContactsChanged {}))
}

pub(crate) async fn stats(services: &Services) -> Result<ContactStatsDto, AppError> {
    Ok(services.contacts.stats().await?.into())
}

impl From<ContactDetail> for ContactDetailDto {
    fn from(detail: ContactDetail) -> Self {
        let metadata = detail.contact.metadata.clone();
        Self {
            contact: detail.contact.into(),
            email_history: detail.email_history.into_iter().map(Into::into).collect(),
            campaigns: detail.campaigns.into_iter().map(Into::into).collect(),
            metadata,
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use weasel_core::enums::{CampaignStatus, ContactStatus, ImportScope};
    use weasel_testkit::builders::parsed_source;

    use crate::commands::testing::{harness, seed_campaign, seed_contact};
    use crate::error::ErrorCode;

    use super::*;

    fn list_request() -> ContactListRequest {
        ContactListRequest {
            search: None,
            statuses: None,
            import_batch_id: None,
            sort: None,
            limit: None,
            offset: None,
        }
    }

    fn create_request(first: &str, last: &str, email: &str) -> ContactCreateRequest {
        ContactCreateRequest {
            first_name: first.to_owned(),
            last_name: last.to_owned(),
            email: email.to_owned(),
            notes: None,
        }
    }

    #[tokio::test]
    async fn list_hides_archived_unless_asked() {
        let harness = harness().await;
        let kept = seed_contact(&harness, "Ada", "Lovelace", "ada@example.com").await;
        let archived = seed_contact(&harness, "Grace", "Hopper", "grace@example.com").await;
        harness
            .services
            .contacts
            .set_status(archived.id, archived.version, ContactStatus::Archived)
            .await
            .unwrap();

        let page = list(&harness.services, list_request()).await.unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].id, kept.id);

        let page = list(
            &harness.services,
            ContactListRequest {
                statuses: Some(vec![ContactStatusDto::Archived]),
                ..list_request()
            },
        )
        .await
        .unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].id, archived.id);
    }

    #[tokio::test]
    async fn list_clamps_oversized_pages() {
        let harness = harness().await;
        let page = list(
            &harness.services,
            ContactListRequest {
                limit: Some(1000),
                ..list_request()
            },
        )
        .await
        .unwrap();
        assert_eq!(page.limit, 500);
    }

    #[tokio::test]
    async fn list_rejects_very_long_searches() {
        let harness = harness().await;
        let error = list(
            &harness.services,
            ContactListRequest {
                search: Some("a".repeat(201)),
                ..list_request()
            },
        )
        .await
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::Validation);
        assert_eq!(error.details.unwrap()["field"], "search");
    }

    #[tokio::test]
    async fn create_emits_one_event_and_a_duplicate_conflicts() {
        let harness = harness().await;
        let emitted = create(
            &harness.services,
            create_request("Ada", "Lovelace", "ada@example.com"),
        )
        .await
        .unwrap();
        assert_eq!(emitted.events, vec![DomainEvent::ContactsChanged {}]);

        let error = create(
            &harness.services,
            create_request("Ada", "Byron", "ADA@example.com"),
        )
        .await
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::Conflict);
    }

    #[tokio::test]
    async fn stale_versions_conflict_and_missing_contacts_are_not_found() {
        let harness = harness().await;
        let contact = seed_contact(&harness, "Ada", "Lovelace", "ada@example.com").await;
        let _ = update(
            &harness.services,
            ContactUpdateRequest {
                id: contact.id,
                version: contact.version,
                first_name: Some("Augusta".to_owned()),
                last_name: None,
                email: None,
                notes: None,
            },
        )
        .await
        .unwrap();

        let error = update(
            &harness.services,
            ContactUpdateRequest {
                id: contact.id,
                version: contact.version,
                first_name: Some("Ada".to_owned()),
                last_name: None,
                email: None,
                notes: None,
            },
        )
        .await
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::Conflict);

        let error = get(&harness.services, ContactGetRequest { id: 9_999 })
            .await
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::NotFound);
    }

    #[tokio::test]
    async fn noop_patch_writes_nothing() {
        let harness = harness().await;
        let contact = seed_contact(&harness, "Ada", "Lovelace", "ada@example.com").await;
        let emitted = update(
            &harness.services,
            ContactUpdateRequest {
                id: contact.id,
                version: contact.version,
                first_name: Some("Ada".to_owned()),
                last_name: Some("Lovelace".to_owned()),
                email: Some(contact.email.clone()),
                notes: Some(contact.notes.clone()),
            },
        )
        .await
        .unwrap();
        assert!(emitted.events.is_empty());
        assert_eq!(emitted.value.version, contact.version);
    }

    #[tokio::test]
    async fn email_changes_keep_two_history_rows() {
        let harness = harness().await;
        let contact = seed_contact(&harness, "Ada", "Lovelace", "ada@example.com").await;
        let emitted = update(
            &harness.services,
            ContactUpdateRequest {
                id: contact.id,
                version: contact.version,
                first_name: None,
                last_name: None,
                email: Some("ada@byron.example".to_owned()),
                notes: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(emitted.events, vec![DomainEvent::ContactsChanged {}]);

        let detail = get(&harness.services, ContactGetRequest { id: contact.id })
            .await
            .unwrap();
        assert_eq!(detail.email_history.len(), 2);
        assert_eq!(detail.email_history[0].email, "ada@byron.example");
        assert_eq!(detail.email_history[0].reason, EmailChangeReasonDto::Manual);
        assert_eq!(detail.email_history[1].email, "ada@example.com");
        assert_eq!(
            detail.email_history[1].valid_to,
            Some(emitted.value.updated_at)
        );
    }

    #[tokio::test]
    async fn contacts_with_send_history_cannot_be_deleted() {
        let harness = harness().await;
        let contact = seed_contact(&harness, "Ada", "Lovelace", "ada@example.com").await;
        let campaign = seed_campaign(&harness, "Launch").await;
        harness
            .services
            .campaigns
            .set_status(campaign.id, campaign.version, CampaignStatus::Active)
            .await
            .unwrap();
        harness
            .services
            .sends
            .prepare_send(campaign.id, None)
            .await
            .unwrap();

        let error = delete(
            &harness.services,
            ContactDeleteRequest {
                id: contact.id,
                version: contact.version + 1,
            },
        )
        .await
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidState);
    }

    #[tokio::test]
    async fn stats_count_imported_rows_by_batch() {
        let harness = harness().await;
        let page = harness
            .services
            .contacts
            .list(&ContactQuery::default())
            .await
            .unwrap();
        assert!(page.items.is_empty());

        let source = parsed_source(&[
            ("Ada", "Lovelace", "ada@example.com"),
            ("Grace", "Hopper", "grace@example.com"),
        ]);
        let preview = harness
            .services
            .imports
            .preview(
                &source,
                weasel_import::service::PreviewOptions {
                    mapping: None,
                    scope: ImportScope::Delta,
                },
                &weasel_core::ports::NoProgress,
            )
            .await
            .unwrap();
        harness
            .services
            .imports
            .apply(preview.batch.id, Vec::new())
            .await
            .unwrap();

        let stats = stats(&harness.services).await.unwrap();
        assert_eq!(stats.active, 2);
        assert_eq!(stats.by_batch.len(), 1);
        assert_eq!(stats.by_batch[0].count, 2);
        assert_eq!(stats.by_batch[0].batch_id, preview.batch.id);
    }
}
