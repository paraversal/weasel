//! Audit trail reads. Mutations are reported by the feature that owns them.

use serde::{Deserialize, Serialize};
use tauri::State;
use weasel_core::CoreError;
use weasel_core::FeatureDescriptor;
use weasel_core::audit::AuditEvent;

use crate::commands::dto::{PageDto, page};
use crate::error::AppError;
use crate::state::{AppState, Services};

/// Entity kinds the activity screen can filter by. They match `audit_events.entity_type`.
const ENTITY_TYPES: &[&str] = &["contact", "campaign", "send_batch", "import_batch"];

#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AuditEventDto {
    pub id: i64,
    pub at: i64,
    pub actor: String,
    pub action: String,
    pub entity_type: String,
    pub entity_id: Option<String>,
    pub before: Option<serde_json::Value>,
    pub after: Option<serde_json::Value>,
    pub metadata: serde_json::Value,
}

impl From<AuditEvent> for AuditEventDto {
    fn from(event: AuditEvent) -> Self {
        Self {
            id: event.id,
            at: event.at,
            actor: event.actor,
            action: event.action,
            entity_type: event.entity_type,
            entity_id: event.entity_id,
            before: event.before,
            after: event.after,
            metadata: event.metadata,
        }
    }
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ActivityListRequest {
    pub entity_type: String,
    pub entity_id: String,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ActivityRecentRequest {
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

pub fn descriptor() -> FeatureDescriptor {
    FeatureDescriptor {
        id: "activity",
        title: "Activity",
        commands: &["activity_list", "activity_recent"],
        events: &[],
    }
}

#[tauri::command]
#[specta::specta]
pub async fn activity_list(
    state: State<'_, AppState>,
    request: ActivityListRequest,
) -> Result<PageDto<AuditEventDto>, AppError> {
    list(state.services()?, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn activity_recent(
    state: State<'_, AppState>,
    request: ActivityRecentRequest,
) -> Result<PageDto<AuditEventDto>, AppError> {
    recent(state.services()?, request).await
}

pub(crate) async fn list(
    services: &Services,
    request: ActivityListRequest,
) -> Result<PageDto<AuditEventDto>, AppError> {
    if !ENTITY_TYPES.contains(&request.entity_type.as_str()) {
        return Err(CoreError::validation(
            "entityType",
            format!(
                "must be one of {}",
                ENTITY_TYPES
                    .iter()
                    .map(|kind| format!("`{kind}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        )
        .into());
    }
    let page = services
        .audit
        .for_entity(
            &request.entity_type,
            &request.entity_id,
            &page(request.limit, request.offset),
        )
        .await?;
    Ok(page.into())
}

pub(crate) async fn recent(
    services: &Services,
    request: ActivityRecentRequest,
) -> Result<PageDto<AuditEventDto>, AppError> {
    let page = services
        .audit
        .recent(&page(request.limit, request.offset))
        .await?;
    Ok(page.into())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use crate::commands::contacts::{ContactUpdateRequest, update};
    use crate::commands::testing::{harness, seed_contact};
    use crate::error::ErrorCode;

    use super::*;

    #[tokio::test]
    async fn entity_history_is_newest_first() {
        let harness = harness().await;
        let contact = seed_contact(&harness, "Ada", "Lovelace", "ada@example.com").await;
        update(
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

        let page = list(
            &harness.services,
            ActivityListRequest {
                entity_type: "contact".to_owned(),
                entity_id: contact.id.to_string(),
                limit: None,
                offset: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(page.total, 2);
        assert_eq!(page.items[0].action, "contact.update");
        assert_eq!(page.items[1].action, "contact.create");
    }

    #[tokio::test]
    async fn recent_returns_every_entity() {
        let harness = harness().await;
        seed_contact(&harness, "Ada", "Lovelace", "ada@example.com").await;
        let page = recent(
            &harness.services,
            ActivityRecentRequest {
                limit: Some(10),
                offset: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].entity_type, "contact");
    }

    #[tokio::test]
    async fn unknown_entity_types_are_validation_errors() {
        let harness = harness().await;
        let error = list(
            &harness.services,
            ActivityListRequest {
                entity_type: "everything".to_owned(),
                entity_id: "1".to_owned(),
                limit: None,
                offset: None,
            },
        )
        .await
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::Validation);
        assert_eq!(error.details.unwrap()["field"], "entityType");
    }
}
