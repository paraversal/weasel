//! Append-only audit trail: one write helper for repositories, one read repository.

use async_trait::async_trait;
use serde_json::Value;
use sqlx::{Row, Sqlite, Transaction};
use weasel_core::{
    CoreResult,
    audit::{AuditEvent, AuditRecord},
    ports::AuditRepository,
    query::{Page, PageRequest},
};

use crate::{
    codec::{decode_json, encode_json},
    error::StoreError,
    pool::Store,
};

/// Persist one audit event inside the caller's transaction.
pub(crate) async fn record(
    tx: &mut Transaction<'_, Sqlite>,
    event: &AuditRecord,
    now_ms: i64,
) -> CoreResult<()> {
    let before = event.before.as_ref().map(encode_json).transpose()?;
    let after = event.after.as_ref().map(encode_json).transpose()?;
    let metadata = encode_json(&event.metadata)?;
    sqlx::query(
        "INSERT INTO audit_events \
         (at, actor, action, entity_type, entity_id, before_json, after_json, metadata_json) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(now_ms)
    .bind(&event.actor)
    .bind(&event.action)
    .bind(&event.entity_type)
    .bind(&event.entity_id)
    .bind(before)
    .bind(after)
    .bind(metadata)
    .execute(&mut **tx)
    .await
    .map_err(StoreError::from)?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct SqliteAudit {
    store: Store,
}

impl SqliteAudit {
    pub fn new(store: Store) -> Self {
        Self { store }
    }

    async fn list(
        &self,
        entity: Option<(&str, &str)>,
        page: PageRequest,
    ) -> CoreResult<Page<AuditEvent>> {
        let page = page.normalized();
        let total: i64 = match entity {
            Some((entity_type, entity_id)) => {
                sqlx::query_scalar(
                    "SELECT COUNT(*) FROM audit_events WHERE entity_type = ? AND entity_id = ?",
                )
                .bind(entity_type)
                .bind(entity_id)
                .fetch_one(self.store.pool())
                .await
                .map_err(StoreError::from)?
            }
            None => sqlx::query_scalar("SELECT COUNT(*) FROM audit_events")
                .fetch_one(self.store.pool())
                .await
                .map_err(StoreError::from)?,
        };

        let sql = if entity.is_some() {
            "SELECT id, at, actor, action, entity_type, entity_id, before_json, after_json, \
             metadata_json FROM audit_events WHERE entity_type = ? AND entity_id = ? \
             ORDER BY at DESC, id DESC LIMIT ? OFFSET ?"
        } else {
            "SELECT id, at, actor, action, entity_type, entity_id, before_json, after_json, \
             metadata_json FROM audit_events ORDER BY at DESC, id DESC LIMIT ? OFFSET ?"
        };
        let mut query = sqlx::query(sql);
        if let Some((entity_type, entity_id)) = entity {
            query = query.bind(entity_type).bind(entity_id);
        }
        let rows = query
            .bind(i64::from(page.limit))
            .bind(i64::from(page.offset))
            .fetch_all(self.store.pool())
            .await
            .map_err(StoreError::from)?;

        let items = rows
            .iter()
            .map(event_from_row)
            .collect::<CoreResult<Vec<_>>>()?;
        Ok(Page {
            items,
            total: u64::try_from(total).unwrap_or_default(),
            offset: page.offset,
            limit: page.limit,
        })
    }
}

#[async_trait]
impl AuditRepository for SqliteAudit {
    async fn for_entity(
        &self,
        entity_type: &str,
        entity_id: &str,
        page: &PageRequest,
    ) -> CoreResult<Page<AuditEvent>> {
        self.list(Some((entity_type, entity_id)), *page).await
    }

    async fn recent(&self, page: &PageRequest) -> CoreResult<Page<AuditEvent>> {
        self.list(None, *page).await
    }
}

fn event_from_row(row: &sqlx::sqlite::SqliteRow) -> CoreResult<AuditEvent> {
    Ok(AuditEvent {
        id: row.try_get("id").map_err(StoreError::from)?,
        at: row.try_get("at").map_err(StoreError::from)?,
        actor: row.try_get("actor").map_err(StoreError::from)?,
        action: row.try_get("action").map_err(StoreError::from)?,
        entity_type: row.try_get("entity_type").map_err(StoreError::from)?,
        entity_id: row.try_get("entity_id").map_err(StoreError::from)?,
        before: decode_optional_json(row, "before_json")?,
        after: decode_optional_json(row, "after_json")?,
        metadata: decode_json(
            "metadata_json",
            &row.try_get::<String, _>("metadata_json")
                .map_err(StoreError::from)?,
        )?,
    })
}

fn decode_optional_json(
    row: &sqlx::sqlite::SqliteRow,
    column: &str,
) -> CoreResult<Option<Value>> {
    let raw: Option<String> = row.try_get(column).map_err(StoreError::from)?;
    raw.as_deref()
        .map(|raw| decode_json(column, raw))
        .transpose()
}
