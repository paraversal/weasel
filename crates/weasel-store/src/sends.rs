//! SQLite send history. `begin_batch` returns an uncommitted batch so the caller can copy
//! the addresses before anything becomes visible.

use async_trait::async_trait;
use sqlx::{Executor, QueryBuilder, Row, Sqlite, Transaction, sqlite::SqliteRow};
use weasel_core::{
    CoreError, CoreResult,
    audit::{AuditRecord, actions},
    campaign::Campaign,
    enums::CampaignStatus,
    ports::{PendingSendBatch, SendRepository},
    query::{Page, PageRequest, like_pattern},
    send::{SendBatch, SendBatchQuery, SendRecord},
};

use crate::{audit, codec::decode_enum, error::StoreError, pool::Store};

const BATCH_COLUMNS: &str = "b.id, b.campaign_id, c.name AS campaign_name, b.created_at, \
     b.recipient_count, b.note, b.copied_at";

const RECORD_COLUMNS: &str = "id, batch_id, contact_id, email, email_normalized, first_name, \
     last_name, status, created_at";

#[derive(Debug, Clone)]
pub struct SqliteSends {
    store: Store,
}

impl SqliteSends {
    pub fn new(store: Store) -> Self {
        Self { store }
    }

    async fn fetch_batch<'e, E>(executor: E, id: i64) -> CoreResult<SendBatch>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let row = sqlx::query(
            "SELECT b.id, b.campaign_id, c.name AS campaign_name, b.created_at, \
             b.recipient_count, b.note, b.copied_at FROM send_batches b \
             JOIN campaigns c ON c.id = b.campaign_id WHERE b.id = ?",
        )
        .bind(id)
        .fetch_optional(executor)
        .await
        .map_err(StoreError::from)?
        .ok_or_else(|| CoreError::NotFound {
            entity: "send_batch",
            id: id.to_string(),
        })?;
        batch_from_row(&row)
    }

    async fn fetch_campaign(tx: &mut Transaction<'_, Sqlite>, id: i64) -> CoreResult<Campaign> {
        let row = sqlx::query(
            "SELECT id, name, description, subject, body, status, created_at, updated_at, \
             version FROM campaigns WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(StoreError::from)?
        .ok_or_else(|| CoreError::NotFound {
            entity: "campaign",
            id: id.to_string(),
        })?;
        Ok(Campaign {
            id: row.try_get("id").map_err(StoreError::from)?,
            name: row.try_get("name").map_err(StoreError::from)?,
            description: row.try_get("description").map_err(StoreError::from)?,
            subject: row.try_get("subject").map_err(StoreError::from)?,
            body: row.try_get("body").map_err(StoreError::from)?,
            status: decode_enum(
                "campaigns.status",
                row.try_get("status").map_err(StoreError::from)?,
            )?,
            created_at: row.try_get("created_at").map_err(StoreError::from)?,
            updated_at: row.try_get("updated_at").map_err(StoreError::from)?,
            version: row.try_get("version").map_err(StoreError::from)?,
        })
    }

    fn push_filters(
        query: &mut QueryBuilder<Sqlite>,
        campaign_id: Option<i64>,
        search: Option<&str>,
    ) {
        if let Some(campaign_id) = campaign_id {
            query.push(" AND b.campaign_id = ").push_bind(campaign_id);
        }
        if let Some(pattern) = search {
            query
                .push(" AND (b.note LIKE ")
                .push_bind(pattern.to_owned())
                .push(" ESCAPE '\\' OR c.name LIKE ")
                .push_bind(pattern.to_owned())
                .push(
                    " ESCAPE '\\' OR EXISTS (SELECT 1 FROM send_records sr \
                        WHERE sr.batch_id = b.id AND (sr.first_name LIKE ",
                )
                .push_bind(pattern.to_owned())
                .push(" ESCAPE '\\' OR sr.last_name LIKE ")
                .push_bind(pattern.to_owned())
                .push(" ESCAPE '\\' OR sr.email LIKE ")
                .push_bind(pattern.to_owned())
                .push(" ESCAPE '\\')))");
        }
    }
}

#[async_trait]
impl SendRepository for SqliteSends {
    async fn begin_batch(
        &self,
        campaign_id: i64,
        now_ms: i64,
    ) -> CoreResult<Box<dyn PendingSendBatch>> {
        let mut tx = self.store.begin_write().await?;
        let campaign = Self::fetch_campaign(&mut tx, campaign_id).await?;
        if campaign.status != CampaignStatus::Active {
            return Err(CoreError::InvalidState(
                "Only active campaigns can be sent".to_owned(),
            ));
        }

        let recipient_rows = sqlx::query(
            "SELECT c.id, c.first_name, c.last_name, c.email, c.email_normalized \
             FROM contacts c WHERE c.status = 'active' AND NOT EXISTS \
             (SELECT 1 FROM send_records r JOIN send_batches b ON b.id = r.batch_id \
              WHERE b.campaign_id = ? AND r.contact_id = c.id) \
             ORDER BY c.last_name COLLATE NOCASE, c.first_name COLLATE NOCASE, c.id",
        )
        .bind(campaign_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(StoreError::from)?;
        if recipient_rows.is_empty() {
            return Err(CoreError::InvalidState(
                "Everyone in the audience has already received this campaign".to_owned(),
            ));
        }

        let batch_id: i64 = sqlx::query_scalar(
            "INSERT INTO send_batches \
             (campaign_id, created_at, recipient_count, note, copied_at) \
             VALUES (?, ?, ?, '', ?) RETURNING id",
        )
        .bind(campaign_id)
        .bind(now_ms)
        .bind(i64::try_from(recipient_rows.len()).unwrap_or(i64::MAX))
        .bind(now_ms)
        .fetch_one(&mut *tx)
        .await
        .map_err(StoreError::from)?;

        let mut emails = Vec::with_capacity(recipient_rows.len());
        for row in &recipient_rows {
            let contact_id: i64 = row.try_get("id").map_err(StoreError::from)?;
            let first_name: String = row.try_get("first_name").map_err(StoreError::from)?;
            let last_name: String = row.try_get("last_name").map_err(StoreError::from)?;
            let email: String = row.try_get("email").map_err(StoreError::from)?;
            let email_normalized: String =
                row.try_get("email_normalized").map_err(StoreError::from)?;
            sqlx::query(
                "INSERT INTO send_records \
                 (batch_id, contact_id, email, email_normalized, first_name, last_name, \
                  status, created_at) VALUES (?, ?, ?, ?, ?, ?, 'copied', ?)",
            )
            .bind(batch_id)
            .bind(contact_id)
            .bind(&email)
            .bind(&email_normalized)
            .bind(&first_name)
            .bind(&last_name)
            .bind(now_ms)
            .execute(&mut *tx)
            .await
            .map_err(StoreError::from)?;
            emails.push(email);
        }

        let batch = SendBatch {
            id: batch_id,
            campaign_id,
            campaign_name: campaign.name,
            created_at: now_ms,
            recipient_count: u64::try_from(recipient_rows.len()).unwrap_or_default(),
            note: String::new(),
            copied_at: Some(now_ms),
        };

        let mut event = AuditRecord::new(actions::SEND_BATCH_CREATE, "send_batch");
        event.entity_id = Some(batch_id.to_string());
        event.after = Some(serde_json::to_value(&batch).map_err(CoreError::internal)?);
        event.metadata = serde_json::json!({
            "campaignId": campaign_id,
            "recipientCount": batch.recipient_count,
        });
        audit::record(&mut tx, &event, now_ms).await?;

        Ok(Box::new(SqlitePendingSendBatch {
            tx: Some(tx),
            batch,
            emails,
        }))
    }

    async fn list(&self, query: &SendBatchQuery) -> CoreResult<Page<SendBatch>> {
        let page = query.page.normalized();
        let search = like_pattern(query.search.as_deref().unwrap_or(""));

        let mut rows_query = QueryBuilder::<Sqlite>::new(format!(
            "SELECT {BATCH_COLUMNS} FROM send_batches b \
             JOIN campaigns c ON c.id = b.campaign_id WHERE 1 = 1"
        ));
        Self::push_filters(&mut rows_query, query.campaign_id, search.as_deref());
        rows_query
            .push(" ORDER BY b.created_at DESC, b.id DESC LIMIT ")
            .push_bind(i64::from(page.limit))
            .push(" OFFSET ")
            .push_bind(i64::from(page.offset));

        let rows = rows_query
            .build()
            .fetch_all(self.store.pool())
            .await
            .map_err(StoreError::from)?;
        let items = rows
            .iter()
            .map(batch_from_row)
            .collect::<CoreResult<Vec<_>>>()?;

        let mut count_query = QueryBuilder::<Sqlite>::new(
            "SELECT COUNT(*) FROM send_batches b JOIN campaigns c ON c.id = b.campaign_id \
             WHERE 1 = 1",
        );
        Self::push_filters(&mut count_query, query.campaign_id, search.as_deref());
        let total: i64 = count_query
            .build_query_scalar()
            .fetch_one(self.store.pool())
            .await
            .map_err(StoreError::from)?;

        Ok(Page {
            items,
            total: u64::try_from(total).unwrap_or_default(),
            offset: page.offset,
            limit: page.limit,
        })
    }

    async fn get(&self, id: i64) -> CoreResult<SendBatch> {
        Self::fetch_batch(self.store.pool(), id).await
    }

    async fn recipients(
        &self,
        id: i64,
        search: Option<&str>,
        page: &PageRequest,
    ) -> CoreResult<Page<SendRecord>> {
        Self::fetch_batch(self.store.pool(), id).await?;
        let page = page.normalized();
        let pattern = like_pattern(search.unwrap_or(""));

        let mut rows_query =
            QueryBuilder::<Sqlite>::new(format!("SELECT {RECORD_COLUMNS} FROM send_records"));
        push_record_filters(&mut rows_query, id, pattern.as_deref());
        rows_query
            .push(" ORDER BY id ASC LIMIT ")
            .push_bind(i64::from(page.limit))
            .push(" OFFSET ")
            .push_bind(i64::from(page.offset));

        let rows = rows_query
            .build()
            .fetch_all(self.store.pool())
            .await
            .map_err(StoreError::from)?;
        let items = rows
            .iter()
            .map(record_from_row)
            .collect::<CoreResult<Vec<_>>>()?;

        let mut count_query = QueryBuilder::<Sqlite>::new("SELECT COUNT(*) FROM send_records");
        push_record_filters(&mut count_query, id, pattern.as_deref());
        let total: i64 = count_query
            .build_query_scalar()
            .fetch_one(self.store.pool())
            .await
            .map_err(StoreError::from)?;

        Ok(Page {
            items,
            total: u64::try_from(total).unwrap_or_default(),
            offset: page.offset,
            limit: page.limit,
        })
    }

    async fn emails(&self, id: i64) -> CoreResult<Vec<String>> {
        Self::fetch_batch(self.store.pool(), id).await?;
        let emails =
            sqlx::query_scalar("SELECT email FROM send_records WHERE batch_id = ? ORDER BY id")
                .bind(id)
                .fetch_all(self.store.pool())
                .await
                .map_err(StoreError::from)?;
        Ok(emails)
    }

    async fn mark_recopied(&self, id: i64, now_ms: i64) -> CoreResult<SendBatch> {
        let mut tx = self.store.begin_write().await?;
        let current = Self::fetch_batch(&mut *tx, id).await?;
        sqlx::query("UPDATE send_batches SET copied_at = ? WHERE id = ?")
            .bind(now_ms)
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(StoreError::from)?;

        let stored = SendBatch {
            copied_at: Some(now_ms),
            ..current.clone()
        };
        let mut event = AuditRecord::new(actions::SEND_BATCH_RECOPY, "send_batch");
        event.entity_id = Some(id.to_string());
        event.before = Some(serde_json::to_value(&current).map_err(CoreError::internal)?);
        event.after = Some(serde_json::to_value(&stored).map_err(CoreError::internal)?);
        audit::record(&mut tx, &event, now_ms).await?;

        tx.commit().await.map_err(StoreError::from)?;
        Ok(stored)
    }

    async fn set_note(&self, id: i64, note: String, now_ms: i64) -> CoreResult<SendBatch> {
        let mut tx = self.store.begin_write().await?;
        let current = Self::fetch_batch(&mut *tx, id).await?;
        sqlx::query("UPDATE send_batches SET note = ? WHERE id = ?")
            .bind(&note)
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(StoreError::from)?;

        let stored = SendBatch {
            note: note.clone(),
            ..current.clone()
        };
        let mut event = AuditRecord::new(actions::SEND_BATCH_NOTE, "send_batch");
        event.entity_id = Some(id.to_string());
        event.before = Some(serde_json::to_value(&current).map_err(CoreError::internal)?);
        event.after = Some(serde_json::to_value(&stored).map_err(CoreError::internal)?);
        event.metadata = serde_json::json!({ "note": note });
        audit::record(&mut tx, &event, now_ms).await?;

        tx.commit().await.map_err(StoreError::from)?;
        Ok(stored)
    }
}

/// Uncommitted batch. Dropping the box drops the transaction, which rolls back.
struct SqlitePendingSendBatch {
    tx: Option<Transaction<'static, Sqlite>>,
    batch: SendBatch,
    emails: Vec<String>,
}

#[async_trait]
impl PendingSendBatch for SqlitePendingSendBatch {
    fn batch(&self) -> &SendBatch {
        &self.batch
    }

    fn emails(&self) -> &[String] {
        &self.emails
    }

    async fn commit(mut self: Box<Self>) -> CoreResult<()> {
        let tx = self.tx.take().ok_or_else(|| {
            CoreError::InvalidState("This send batch was already committed".to_owned())
        })?;
        tx.commit().await.map_err(StoreError::from)?;
        Ok(())
    }
}

fn push_record_filters(query: &mut QueryBuilder<Sqlite>, batch_id: i64, search: Option<&str>) {
    query.push(" WHERE batch_id = ").push_bind(batch_id);
    if let Some(pattern) = search {
        query
            .push(" AND (first_name LIKE ")
            .push_bind(pattern.to_owned())
            .push(" ESCAPE '\\' OR last_name LIKE ")
            .push_bind(pattern.to_owned())
            .push(" ESCAPE '\\' OR email LIKE ")
            .push_bind(pattern.to_owned())
            .push(" ESCAPE '\\')");
    }
}

fn batch_from_row(row: &SqliteRow) -> CoreResult<SendBatch> {
    Ok(SendBatch {
        id: row.try_get("id").map_err(StoreError::from)?,
        campaign_id: row.try_get("campaign_id").map_err(StoreError::from)?,
        campaign_name: row.try_get("campaign_name").map_err(StoreError::from)?,
        created_at: row.try_get("created_at").map_err(StoreError::from)?,
        recipient_count: u64::try_from(
            row.try_get::<i64, _>("recipient_count")
                .map_err(StoreError::from)?,
        )
        .unwrap_or_default(),
        note: row.try_get("note").map_err(StoreError::from)?,
        copied_at: row.try_get("copied_at").map_err(StoreError::from)?,
    })
}

fn record_from_row(row: &SqliteRow) -> CoreResult<SendRecord> {
    Ok(SendRecord {
        id: row.try_get("id").map_err(StoreError::from)?,
        batch_id: row.try_get("batch_id").map_err(StoreError::from)?,
        contact_id: row.try_get("contact_id").map_err(StoreError::from)?,
        email: row.try_get("email").map_err(StoreError::from)?,
        first_name: row.try_get("first_name").map_err(StoreError::from)?,
        last_name: row.try_get("last_name").map_err(StoreError::from)?,
        status: decode_enum(
            "send_records.status",
            row.try_get("status").map_err(StoreError::from)?,
        )?,
        created_at: row.try_get("created_at").map_err(StoreError::from)?,
    })
}
