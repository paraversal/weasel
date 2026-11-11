//! Import batches: preview drafts, application and contact snapshots.

use async_trait::async_trait;
use serde_json::json;
use sqlx::sqlite::SqliteRow;
use sqlx::{QueryBuilder, Row, Sqlite};
use weasel_core::audit::{AuditRecord, actions};
use weasel_core::contact::{Contact, ContactChanges, ContactQuery, ContactSort, NewContactRecord};
use weasel_core::enums::{ContactSource, ContactStatus, EmailChangeReason, ImportStatus};
use weasel_core::import::{
    ContactOp, ContactRef, ImportBatch, ImportPlan, ImportSummary, NewImportBatch,
    ReconcileSnapshot,
};
use weasel_core::ports::{ContactRepository, ImportRepository};
use weasel_core::query::{Page, PageRequest};
use weasel_core::{CoreError, CoreResult};

use crate::Store;
use crate::audit;
use crate::codec::{decode_enum, decode_json, encode_json};
use crate::contacts::{SqliteContacts, write_contact_update, write_new_contact};
use crate::error::StoreError;

const IMPORT_COLUMNS: &str = "id, source_kind, source_name, source_path, file_hash, sheet_name, \
     scope, status, created_at, applied_at, row_count, added_count, updated_count, removed_count, \
     conflict_count, unchanged_count, invalid_count, error";

const APPLY_STALE: &str = "Contacts changed since the preview. Preview the file again.";

#[derive(Debug, Clone)]
pub struct SqliteImports {
    store: Store,
}

impl SqliteImports {
    pub fn new(store: Store) -> Self {
        Self { store }
    }

    async fn fetch(&self, id: i64) -> CoreResult<ImportBatch> {
        let row = sqlx::query(
            "SELECT id, source_kind, source_name, source_path, file_hash, sheet_name, \
             scope, status, created_at, applied_at, row_count, added_count, updated_count, \
             removed_count, conflict_count, unchanged_count, invalid_count, error \
             FROM import_batches WHERE id = ?",
        )
        .bind(id)
        .fetch_one(self.store.pool())
        .await
        .map_err(StoreError::from)?;
        batch_from_row(&row)
    }

    async fn list_status(
        &self,
        status: Option<ImportStatus>,
        page: &PageRequest,
    ) -> CoreResult<Page<ImportBatch>> {
        let mut builder: QueryBuilder<Sqlite> =
            QueryBuilder::new(format!("SELECT {IMPORT_COLUMNS} FROM import_batches"));
        if let Some(status) = status {
            builder.push(" WHERE status = ").push_bind(status.as_str());
        }
        builder
            .push(" ORDER BY created_at DESC, id DESC LIMIT ")
            .push_bind(page.limit as i64)
            .push(" OFFSET ")
            .push_bind(page.offset as i64);
        let rows = builder
            .build()
            .fetch_all(self.store.pool())
            .await
            .map_err(StoreError::from)?;
        let items = rows
            .iter()
            .map(batch_from_row)
            .collect::<CoreResult<Vec<_>>>()?;

        let mut count: QueryBuilder<Sqlite> =
            QueryBuilder::new("SELECT COUNT(*) FROM import_batches");
        if let Some(status) = status {
            count.push(" WHERE status = ").push_bind(status.as_str());
        }
        let total: i64 = count
            .build_query_scalar()
            .fetch_one(self.store.pool())
            .await
            .map_err(StoreError::from)?;

        Ok(Page {
            items,
            total: total.max(0) as u64,
            limit: page.limit,
            offset: page.offset,
        })
    }
}

#[async_trait]
impl ImportRepository for SqliteImports {
    async fn snapshot(&self) -> CoreResult<ReconcileSnapshot> {
        let rows = sqlx::query(
            "SELECT id, version, first_name, last_name, email, email_normalized, identity_key, \
             status, source FROM contacts WHERE status IN ('active', 'removed') ORDER BY id",
        )
        .fetch_all(self.store.pool())
        .await
        .map_err(StoreError::from)?;
        let contacts = rows
            .iter()
            .map(contact_ref_from_row)
            .collect::<CoreResult<Vec<_>>>()?;

        let history_rows = sqlx::query(
            "SELECT h.email_normalized, h.contact_id FROM contact_email_history h \
             JOIN contacts c ON c.id = h.contact_id \
             WHERE c.status IN ('active', 'removed') ORDER BY h.contact_id, h.valid_from",
        )
        .fetch_all(self.store.pool())
        .await
        .map_err(StoreError::from)?;
        let history = history_rows
            .iter()
            .map(|row| {
                Ok((
                    row.try_get("email_normalized").map_err(StoreError::from)?,
                    row.try_get("contact_id").map_err(StoreError::from)?,
                ))
            })
            .collect::<CoreResult<Vec<_>>>()?;

        Ok(ReconcileSnapshot { contacts, history })
    }

    async fn create_draft(&self, draft: NewImportBatch, now_ms: i64) -> CoreResult<ImportBatch> {
        let plan_json = encode_json(&draft.plan)?;
        let summary = &draft.summary;
        let mut tx = self.store.begin_write().await?;
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO import_batches \
             (source_kind, source_name, source_path, file_hash, sheet_name, scope, status, \
              created_at, row_count, added_count, updated_count, removed_count, conflict_count, \
              unchanged_count, invalid_count, plan_json) \
             VALUES (?, ?, ?, ?, ?, ?, 'draft', ?, ?, ?, ?, ?, ?, ?, ?, ?) \
             RETURNING id",
        )
        .bind(draft.source_kind.as_str())
        .bind(&draft.source_name)
        .bind(&draft.source_path)
        .bind(&draft.file_hash)
        .bind(&draft.sheet_name)
        .bind(draft.scope.as_str())
        .bind(now_ms)
        .bind(summary.row_count as i64)
        .bind(summary.added as i64)
        .bind(summary.updated as i64)
        .bind(summary.removed as i64)
        .bind(summary.conflicts as i64)
        .bind(summary.unchanged as i64)
        .bind(summary.invalid as i64)
        .bind(&plan_json)
        .fetch_one(&mut *tx)
        .await
        .map_err(StoreError::from)?;

        let mut event = AuditRecord::new(actions::IMPORT_PREVIEW, "import_batch");
        event.entity_id = Some(id.to_string());
        event.metadata = json!({
            "scope": draft.scope.as_str(),
            "fileHash": draft.file_hash,
            "summary": summary,
        });
        audit::record(&mut tx, &event, now_ms).await?;

        tx.commit().await.map_err(StoreError::from)?;
        self.fetch(id).await
    }

    async fn get(&self, id: i64) -> CoreResult<ImportBatch> {
        self.fetch(id).await
    }

    async fn plan(&self, id: i64) -> CoreResult<ImportPlan> {
        let plan_json: Option<String> =
            sqlx::query_scalar("SELECT plan_json FROM import_batches WHERE id = ?")
                .bind(id)
                .fetch_one(self.store.pool())
                .await
                .map_err(StoreError::from)?;
        let Some(plan_json) = plan_json else {
            return Err(CoreError::InvalidState(
                "This import has no stored plan. Preview the file again.".to_owned(),
            ));
        };
        let plan: ImportPlan = decode_json("import_batches.plan_json", &plan_json)?;
        if plan.version != weasel_core::import::PLAN_VERSION {
            return Err(CoreError::InvalidState("Preview the file again".to_owned()));
        }
        Ok(plan)
    }

    async fn list(
        &self,
        status: Option<ImportStatus>,
        page: &PageRequest,
    ) -> CoreResult<Page<ImportBatch>> {
        self.list_status(status, page).await
    }

    async fn members(&self, id: i64, page: &PageRequest) -> CoreResult<Page<Contact>> {
        let contacts = SqliteContacts::new(self.store.clone());
        contacts
            .list(&ContactQuery {
                on_hold: false,
                search: None,
                statuses: vec![
                    ContactStatus::Active,
                    ContactStatus::Removed,
                    ContactStatus::Archived,
                ],
                import_batch_id: Some(id),
                sort: ContactSort::AddedDesc,
                page: *page,
            })
            .await
    }

    async fn last_applied_with_hash(&self, hash: &str) -> CoreResult<Option<i64>> {
        let id = sqlx::query_scalar(
            "SELECT id FROM import_batches \
             WHERE file_hash = ? AND status = 'applied' \
             ORDER BY COALESCE(applied_at, created_at) DESC, id DESC LIMIT 1",
        )
        .bind(hash)
        .fetch_optional(self.store.pool())
        .await
        .map_err(StoreError::from)?;
        Ok(id)
    }

    async fn apply(
        &self,
        id: i64,
        ops: Vec<ContactOp>,
        summary: ImportSummary,
        now_ms: i64,
    ) -> CoreResult<ImportBatch> {
        let mut tx = self.store.begin_write().await?;

        let scope: Option<String> =
            sqlx::query_scalar("SELECT scope FROM import_batches WHERE id = ?")
                .bind(id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(StoreError::from)?;
        let Some(scope) = scope else {
            return Err(CoreError::NotFound {
                entity: "import_batch",
                id: id.to_string(),
            });
        };

        let marked = sqlx::query(
            "UPDATE import_batches SET status = 'applied' WHERE id = ? AND status = 'draft'",
        )
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(StoreError::from)?;
        if marked.rows_affected() == 0 {
            return Err(CoreError::InvalidState(
                "Import was already applied or discarded".to_owned(),
            ));
        }

        for op in ops {
            apply_op(&mut tx, id, op, now_ms).await?;
        }

        sqlx::query(
            "UPDATE import_batches SET applied_at = ?, row_count = ?, added_count = ?, \
             updated_count = ?, removed_count = ?, conflict_count = ?, unchanged_count = ?, \
             invalid_count = ? WHERE id = ?",
        )
        .bind(now_ms)
        .bind(summary.row_count as i64)
        .bind(summary.added as i64)
        .bind(summary.updated as i64)
        .bind(summary.removed as i64)
        .bind(summary.conflicts as i64)
        .bind(summary.unchanged as i64)
        .bind(summary.invalid as i64)
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(StoreError::from)?;

        let file_hash: Option<String> =
            sqlx::query_scalar("SELECT file_hash FROM import_batches WHERE id = ?")
                .bind(id)
                .fetch_one(&mut *tx)
                .await
                .map_err(StoreError::from)?;
        let mut event = AuditRecord::new(actions::IMPORT_APPLY, "import_batch");
        event.entity_id = Some(id.to_string());
        event.metadata = json!({
            "scope": scope,
            "fileHash": file_hash,
            "summary": summary,
        });
        audit::record(&mut tx, &event, now_ms).await?;

        tx.commit().await.map_err(StoreError::from)?;
        self.fetch(id).await
    }

    async fn discard(&self, id: i64, now_ms: i64) -> CoreResult<ImportBatch> {
        let mut tx = self.store.begin_write().await?;
        let discarded = sqlx::query(
            "UPDATE import_batches SET status = 'discarded', plan_json = NULL \
             WHERE id = ? AND status = 'draft'",
        )
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(StoreError::from)?;
        if discarded.rows_affected() == 0 {
            let exists: bool =
                sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM import_batches WHERE id = ?)")
                    .bind(id)
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(StoreError::from)?;
            if !exists {
                return Err(CoreError::NotFound {
                    entity: "import_batch",
                    id: id.to_string(),
                });
            }
            return Err(CoreError::InvalidState(
                "Only draft imports can be discarded".to_owned(),
            ));
        }

        let mut event = AuditRecord::new(actions::IMPORT_DISCARD, "import_batch");
        event.entity_id = Some(id.to_string());
        audit::record(&mut tx, &event, now_ms).await?;

        tx.commit().await.map_err(StoreError::from)?;
        self.fetch(id).await
    }

    async fn count_drafts(&self) -> CoreResult<u64> {
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM import_batches WHERE status = 'draft'")
                .fetch_one(self.store.pool())
                .await
                .map_err(StoreError::from)?;
        Ok(count.max(0) as u64)
    }
}

async fn apply_op(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    batch_id: i64,
    op: ContactOp,
    now_ms: i64,
) -> CoreResult<()> {
    match op {
        ContactOp::Create {
            row,
            first_name,
            last_name,
            email,
            identity_key,
        } => {
            let stored = write_new_contact(
                tx,
                NewContactRecord {
                    first_name,
                    last_name,
                    email,
                    notes: String::new(),
                    source: ContactSource::Import,
                    identity_key,
                },
                Some(batch_id),
                now_ms,
            )
            .await?;
            let mut event = AuditRecord::new(actions::CONTACT_IMPORT_CREATE, "contact");
            event.entity_id = Some(stored.id.to_string());
            event.after = Some(serde_json::to_value(&stored).map_err(CoreError::internal)?);
            event.metadata = json!({ "importBatchId": batch_id, "row": row });
            audit::record(tx, &event, now_ms).await?;
        }
        ContactOp::Modify {
            row,
            contact_id,
            expected_version,
            first_name,
            last_name,
            email,
            status,
            identity_key,
        } => {
            let changes = ContactChanges {
                first_name,
                last_name,
                email,
                notes: None,
                identity_key,
            };
            let (before, stored) = write_contact_update(
                tx,
                contact_id,
                expected_version,
                changes,
                status,
                EmailChangeReason::Import,
                now_ms,
                APPLY_STALE.to_owned(),
                true,
            )
            .await?;
            let mut event = AuditRecord::new(actions::CONTACT_IMPORT_UPDATE, "contact");
            event.entity_id = Some(contact_id.to_string());
            event.before = Some(serde_json::to_value(&before).map_err(CoreError::internal)?);
            event.after = Some(serde_json::to_value(&stored).map_err(CoreError::internal)?);
            event.metadata = json!({ "importBatchId": batch_id, "row": row });
            audit::record(tx, &event, now_ms).await?;
        }
    }
    Ok(())
}

fn batch_from_row(row: &SqliteRow) -> CoreResult<ImportBatch> {
    Ok(ImportBatch {
        id: row.try_get("id").map_err(StoreError::from)?,
        source_kind: decode_enum(
            "import_batches.source_kind",
            row.try_get("source_kind").map_err(StoreError::from)?,
        )?,
        source_name: row.try_get("source_name").map_err(StoreError::from)?,
        source_path: row.try_get("source_path").map_err(StoreError::from)?,
        file_hash: row.try_get("file_hash").map_err(StoreError::from)?,
        sheet_name: row.try_get("sheet_name").map_err(StoreError::from)?,
        scope: decode_enum(
            "import_batches.scope",
            row.try_get("scope").map_err(StoreError::from)?,
        )?,
        status: decode_enum(
            "import_batches.status",
            row.try_get("status").map_err(StoreError::from)?,
        )?,
        created_at: row.try_get("created_at").map_err(StoreError::from)?,
        applied_at: row.try_get("applied_at").map_err(StoreError::from)?,
        summary: ImportSummary {
            row_count: count(row, "row_count")?,
            added: count(row, "added_count")?,
            updated: count(row, "updated_count")?,
            removed: count(row, "removed_count")?,
            conflicts: count(row, "conflict_count")?,
            unchanged: count(row, "unchanged_count")?,
            invalid: count(row, "invalid_count")?,
        },
        error: row.try_get("error").map_err(StoreError::from)?,
    })
}

fn count(row: &SqliteRow, column: &str) -> CoreResult<u64> {
    let value: i64 = row.try_get(column).map_err(StoreError::from)?;
    Ok(value.max(0) as u64)
}

fn contact_ref_from_row(row: &SqliteRow) -> CoreResult<ContactRef> {
    Ok(ContactRef {
        id: row.try_get("id").map_err(StoreError::from)?,
        version: row.try_get("version").map_err(StoreError::from)?,
        first_name: row.try_get("first_name").map_err(StoreError::from)?,
        last_name: row.try_get("last_name").map_err(StoreError::from)?,
        email: row.try_get("email").map_err(StoreError::from)?,
        email_normalized: row.try_get("email_normalized").map_err(StoreError::from)?,
        identity_key: row.try_get("identity_key").map_err(StoreError::from)?,
        status: decode_enum(
            "contacts.status",
            row.try_get("status").map_err(StoreError::from)?,
        )?,
        source: decode_enum(
            "contacts.source",
            row.try_get("source").map_err(StoreError::from)?,
        )?,
    })
}
