//! SQLite contact repository. Every write runs in one `BEGIN IMMEDIATE` transaction and
//! records its own audit row.

use async_trait::async_trait;
use serde_json::Value;
use sqlx::{
    Executor, QueryBuilder, Row, Sqlite,
    sqlite::SqliteRow,
};
use weasel_core::{
    CoreError, CoreResult,
    audit::{AuditRecord, actions},
    contact::{
        BatchCount, CampaignReceipt, Contact, ContactChanges, ContactDetail, ContactQuery,
        ContactSort, ContactStats, EmailHistoryEntry, NewContactRecord,
    },
    enums::{ContactSource, ContactStatus, EmailChangeReason},
    identity::NAME_IDENTITY_ID,
    ports::ContactRepository,
    query::Page,
};

use crate::{
    audit,
    codec::decode_enum,
    error::StoreError,
    pool::Store,
};

const CONTACT_COLUMNS: &str = "id, first_name, last_name, email, email_normalized, \
     identity_key, status, source, import_batch_id, date_added, updated_at, version, notes, \
     metadata_json";

#[derive(Debug, Clone)]
pub struct SqliteContacts {
    store: Store,
}

impl SqliteContacts {
    pub fn new(store: Store) -> Self {
        Self { store }
    }

    async fn fetch_one<'e, E>(executor: E, id: i64) -> CoreResult<Contact>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let row = sqlx::query(
            "SELECT id, first_name, last_name, email, email_normalized, identity_key, status, \
             source, import_batch_id, date_added, updated_at, version, notes, metadata_json \
             FROM contacts WHERE id = ?",
        )
        .bind(id)
            .fetch_optional(executor)
            .await
            .map_err(StoreError::from)?
            .ok_or_else(|| CoreError::NotFound {
                entity: "contact",
                id: id.to_string(),
            })?;
        contact_from_row(&row)
    }
}

#[async_trait]
impl ContactRepository for SqliteContacts {
    async fn list(&self, query: &ContactQuery) -> CoreResult<Page<Contact>> {
        let page = query.page.normalized();
        let statuses = query.effective_statuses();
        let search = weasel_core::query::like_pattern(query.search.as_deref().unwrap_or(""));

        let mut rows_query =
            QueryBuilder::<Sqlite>::new(format!("SELECT {CONTACT_COLUMNS} FROM contacts"));
        push_filters(
            &mut rows_query,
            &statuses,
            search.as_deref(),
            query.import_batch_id,
        );
        rows_query.push(match query.sort {
            ContactSort::LastNameAsc => {
                " ORDER BY last_name COLLATE NOCASE ASC, first_name COLLATE NOCASE ASC, id ASC"
            }
            ContactSort::LastNameDesc => {
                " ORDER BY last_name COLLATE NOCASE DESC, first_name COLLATE NOCASE ASC, id ASC"
            }
            ContactSort::AddedDesc => " ORDER BY date_added DESC, id DESC",
            ContactSort::EmailAsc => " ORDER BY email_normalized ASC, id ASC",
        });
        rows_query
            .push(" LIMIT ")
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
            .map(contact_from_row)
            .collect::<CoreResult<Vec<_>>>()?;

        let mut count_query = QueryBuilder::<Sqlite>::new("SELECT COUNT(*) FROM contacts");
        push_filters(
            &mut count_query,
            &statuses,
            search.as_deref(),
            query.import_batch_id,
        );
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

    async fn get(&self, id: i64) -> CoreResult<Contact> {
        Self::fetch_one(self.store.pool(), id).await
    }

    async fn detail(&self, id: i64) -> CoreResult<ContactDetail> {
        let contact = self.get(id).await?;

        let history_rows = sqlx::query(
            "SELECT email, valid_from, valid_to, reason FROM contact_email_history \
             WHERE contact_id = ? ORDER BY valid_from DESC, id DESC",
        )
        .bind(id)
        .fetch_all(self.store.pool())
        .await
        .map_err(StoreError::from)?;
        let email_history = history_rows
            .iter()
            .map(|row| {
                Ok(EmailHistoryEntry {
                    email: row.try_get("email").map_err(StoreError::from)?,
                    valid_from: row.try_get("valid_from").map_err(StoreError::from)?,
                    valid_to: row.try_get("valid_to").map_err(StoreError::from)?,
                    reason: decode_enum(
                        "contact_email_history.reason",
                        row.try_get("reason").map_err(StoreError::from)?,
                    )?,
                })
            })
            .collect::<CoreResult<Vec<_>>>()?;

        let receipt_rows = sqlx::query(
            "SELECT b.campaign_id, c.name, r.batch_id, b.created_at \
             FROM send_records r \
             JOIN send_batches b ON b.id = r.batch_id \
             JOIN campaigns c ON c.id = b.campaign_id \
             WHERE r.contact_id = ? \
             ORDER BY b.created_at DESC, r.id DESC",
        )
        .bind(id)
        .fetch_all(self.store.pool())
        .await
        .map_err(StoreError::from)?;
        let campaigns = receipt_rows
            .iter()
            .map(|row| {
                Ok(CampaignReceipt {
                    campaign_id: row.try_get("campaign_id").map_err(StoreError::from)?,
                    campaign_name: row.try_get("name").map_err(StoreError::from)?,
                    batch_id: row.try_get("batch_id").map_err(StoreError::from)?,
                    sent_at: row.try_get("created_at").map_err(StoreError::from)?,
                })
            })
            .collect::<CoreResult<Vec<_>>>()?;

        Ok(ContactDetail {
            contact,
            email_history,
            campaigns,
        })
    }

    async fn stats(&self) -> CoreResult<ContactStats> {
        let counts = sqlx::query("SELECT status, COUNT(*) AS count FROM contacts GROUP BY status")
            .fetch_all(self.store.pool())
            .await
            .map_err(StoreError::from)?;
        let mut stats = ContactStats::default();
        for row in &counts {
            let status: ContactStatus =
                decode_enum("contacts.status", row.try_get("status").map_err(StoreError::from)?)?;
            let count = u64::try_from(row.try_get::<i64, _>("count").map_err(StoreError::from)?)
                .unwrap_or_default();
            match status {
                ContactStatus::Active => stats.active = count,
                ContactStatus::Removed => stats.removed = count,
                ContactStatus::Archived => stats.archived = count,
            }
            stats.total += count;
        }

        let batch_rows = sqlx::query(
            "SELECT c.import_batch_id AS batch_id, ib.source_name AS source_name, \
             COUNT(*) AS count \
             FROM contacts c JOIN import_batches ib ON ib.id = c.import_batch_id \
             GROUP BY c.import_batch_id, ib.source_name \
             ORDER BY c.import_batch_id DESC",
        )
        .fetch_all(self.store.pool())
        .await
        .map_err(StoreError::from)?;
        stats.by_batch = batch_rows
            .iter()
            .map(|row| {
                Ok(BatchCount {
                    batch_id: row.try_get("batch_id").map_err(StoreError::from)?,
                    source_name: row.try_get("source_name").map_err(StoreError::from)?,
                    count: u64::try_from(
                        row.try_get::<i64, _>("count").map_err(StoreError::from)?,
                    )
                    .unwrap_or_default(),
                })
            })
            .collect::<CoreResult<Vec<_>>>()?;
        Ok(stats)
    }

    async fn count_active(&self) -> CoreResult<u64> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM contacts WHERE status = 'active'")
            .fetch_one(self.store.pool())
            .await
            .map_err(StoreError::from)?;
        Ok(u64::try_from(count).unwrap_or_default())
    }

    async fn create(&self, contact: NewContactRecord, now_ms: i64) -> CoreResult<Contact> {
        let mut tx = self.store.begin_write().await?;
        let stored = write_new_contact(&mut tx, contact, None, now_ms).await?;

        let action = match stored.source {
            ContactSource::Import => actions::CONTACT_IMPORT_CREATE,
            _ => actions::CONTACT_CREATE,
        };
        let mut event = AuditRecord::new(action, "contact");
        event.entity_id = Some(stored.id.to_string());
        event.after = Some(serde_json::to_value(&stored).map_err(CoreError::internal)?);
        audit::record(&mut tx, &event, now_ms).await?;

        tx.commit().await.map_err(StoreError::from)?;
        Ok(stored)
    }

    async fn update(
        &self,
        id: i64,
        expected_version: i64,
        changes: ContactChanges,
        reason: EmailChangeReason,
        now_ms: i64,
    ) -> CoreResult<Contact> {
        let mut tx = self.store.begin_write().await?;
        let (current, stored) = write_contact_update(
            &mut tx,
            id,
            expected_version,
            changes,
            None,
            reason,
            now_ms,
            format!(
                "This contact was changed elsewhere. Reload and try again (expected version \
                 {expected_version})."
            ),
            false,
        )
        .await?;
        let email_changed = stored.email_normalized != current.email_normalized;

        let mut event = AuditRecord::new(
            if email_changed {
                actions::CONTACT_EMAIL_CHANGE
            } else {
                actions::CONTACT_UPDATE
            },
            "contact",
        );
        event.entity_id = Some(id.to_string());
        event.before = Some(serde_json::to_value(&current).map_err(CoreError::internal)?);
        event.after = Some(serde_json::to_value(&stored).map_err(CoreError::internal)?);
        if email_changed {
            event.metadata = serde_json::json!({ "reason": reason.as_str() });
        }
        audit::record(&mut tx, &event, now_ms).await?;

        tx.commit().await.map_err(StoreError::from)?;
        Ok(stored)
    }

    async fn set_status(
        &self,
        id: i64,
        expected_version: i64,
        status: ContactStatus,
        now_ms: i64,
    ) -> CoreResult<Contact> {
        let mut tx = self.store.begin_write().await?;
        let current = Self::fetch_one(&mut *tx, id).await?;

        let updated = sqlx::query(
            "UPDATE contacts SET status = ?, version = version + 1, updated_at = ? \
             WHERE id = ? AND version = ?",
        )
        .bind(status.as_str())
        .bind(now_ms)
        .bind(id)
        .bind(expected_version)
        .execute(&mut *tx)
        .await
        .map_err(StoreError::from)?;
        if updated.rows_affected() == 0 {
            return Err(stale_version(&mut tx, id, expected_version).await);
        }

        let before = serde_json::to_value(&current).map_err(CoreError::internal)?;
        let stored = Contact {
            status,
            updated_at: now_ms,
            version: current.version + 1,
            ..current
        };
        let mut event = AuditRecord::new(actions::CONTACT_STATUS, "contact");
        event.entity_id = Some(id.to_string());
        event.before = Some(before);
        event.after = Some(serde_json::to_value(&stored).map_err(CoreError::internal)?);
        audit::record(&mut tx, &event, now_ms).await?;

        tx.commit().await.map_err(StoreError::from)?;
        Ok(stored)
    }

    async fn delete(&self, id: i64, expected_version: i64, now_ms: i64) -> CoreResult<()> {
        let mut tx = self.store.begin_write().await?;
        let current = Self::fetch_one(&mut *tx, id).await?;

        let has_sends: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM send_records WHERE contact_id = ?)",
        )
        .bind(id)
        .fetch_one(&mut *tx)
        .await
        .map_err(StoreError::from)?;
        if has_sends {
            return Err(CoreError::InvalidState(
                "This person has send history; archive instead".to_owned(),
            ));
        }

        let deleted = sqlx::query("DELETE FROM contacts WHERE id = ? AND version = ?")
            .bind(id)
            .bind(expected_version)
            .execute(&mut *tx)
            .await
            .map_err(StoreError::from)?;
        if deleted.rows_affected() == 0 {
            return Err(stale_version(&mut tx, id, expected_version).await);
        }

        let mut event = AuditRecord::new(actions::CONTACT_DELETE, "contact");
        event.entity_id = Some(id.to_string());
        event.before = Some(serde_json::to_value(&current).map_err(CoreError::internal)?);
        audit::record(&mut tx, &event, now_ms).await?;

        tx.commit().await.map_err(StoreError::from)?;
        Ok(())
    }
}

/// A versioned write that matched nothing is either a missing row or a stale version.
async fn stale_version(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    id: i64,
    expected_version: i64,
) -> CoreError {
    let exists: Option<i64> = sqlx::query_scalar("SELECT 1 FROM contacts WHERE id = ?")
        .bind(id)
        .fetch_optional(&mut **tx)
        .await
        .ok()
        .flatten();
    if exists.is_none() {
        CoreError::NotFound {
            entity: "contact",
            id: id.to_string(),
        }
    } else {
        CoreError::Conflict(format!(
            "This contact was changed elsewhere. Reload and try again (expected version \
             {expected_version})."
        ))
    }
}

fn push_filters(
    query: &mut QueryBuilder<Sqlite>,
    statuses: &[ContactStatus],
    search: Option<&str>,
    batch_id: Option<i64>,
) {
    query.push(" WHERE status IN (");
    {
        let mut separated = query.separated(", ");
        for status in statuses {
            separated.push_bind(status.as_str());
        }
    }
    query.push(")");
    if let Some(pattern) = search {
        query
            .push(" AND (first_name LIKE ")
            .push_bind(pattern.to_owned())
            .push(" ESCAPE '\\' OR last_name LIKE ")
            .push_bind(pattern.to_owned())
            .push(" ESCAPE '\\' OR email_normalized LIKE ")
            .push_bind(pattern.to_owned())
            .push(" ESCAPE '\\')");
    }
    if let Some(batch_id) = batch_id {
        query.push(" AND import_batch_id = ").push_bind(batch_id);
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn write_new_contact(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    contact: NewContactRecord,
    import_batch_id: Option<i64>,
    now_ms: i64,
) -> CoreResult<Contact> {
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO contacts \
         (first_name, last_name, email, email_normalized, identity_key, status, source, \
          import_batch_id, date_added, updated_at, version, notes, metadata_json) \
         VALUES (?, ?, ?, ?, ?, 'active', ?, ?, ?, ?, 1, ?, '{}') \
         RETURNING id",
    )
    .bind(&contact.first_name)
    .bind(&contact.last_name)
    .bind(&contact.email.display)
    .bind(&contact.email.normalized)
    .bind(&contact.identity_key)
    .bind(contact.source.as_str())
    .bind(import_batch_id)
    .bind(now_ms)
    .bind(now_ms)
    .bind(&contact.notes)
    .fetch_one(&mut **tx)
    .await
    .map_err(StoreError::from)?;

    sqlx::query(
        "INSERT INTO contact_email_history \
         (contact_id, email, email_normalized, valid_from, reason) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(&contact.email.display)
    .bind(&contact.email.normalized)
    .bind(now_ms)
    .bind(create_reason(contact.source).as_str())
    .execute(&mut **tx)
    .await
    .map_err(StoreError::from)?;

    // First domain write records which identity strategy produced `identity_key`.
    sqlx::query(
        "INSERT INTO app_meta (key, value) VALUES ('identity_strategy', ?) \
         ON CONFLICT(key) DO NOTHING",
    )
    .bind(NAME_IDENTITY_ID)
    .execute(&mut **tx)
    .await
    .map_err(StoreError::from)?;

    Ok(Contact {
        id,
        first_name: contact.first_name,
        last_name: contact.last_name,
        email: contact.email.display,
        email_normalized: contact.email.normalized,
        identity_key: contact.identity_key,
        status: ContactStatus::Active,
        source: contact.source,
        import_batch_id,
        date_added: now_ms,
        updated_at: now_ms,
        version: 1,
        notes: contact.notes,
        metadata: Value::Object(Default::default()),
    })
}

/// Applies a versioned contact update and maintains the open email-history row.
///
/// Returns `(before, after)`. `stale_message` is used both for a version miss and,
/// when `missing_is_conflict` is set, for a deleted target.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn write_contact_update(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    id: i64,
    expected_version: i64,
    changes: ContactChanges,
    status: Option<ContactStatus>,
    reason: EmailChangeReason,
    now_ms: i64,
    stale_message: String,
    missing_is_conflict: bool,
) -> CoreResult<(Contact, Contact)> {
    let current = SqliteContacts::fetch_one(&mut **tx, id).await?;

    let first_name = changes
        .first_name
        .clone()
        .unwrap_or(current.first_name.clone());
    let last_name = changes.last_name.clone().unwrap_or(current.last_name.clone());
    let (email, email_normalized) = match &changes.email {
        Some(email) => (email.display.clone(), email.normalized.clone()),
        None => (current.email.clone(), current.email_normalized.clone()),
    };
    let notes = changes.notes.clone().unwrap_or(current.notes.clone());
    let names_changed = changes.first_name.is_some() || changes.last_name.is_some();
    let identity_key = if names_changed {
        changes.identity_key.clone()
    } else {
        current.identity_key.clone()
    };
    let email_changed = email_normalized != current.email_normalized || email != current.email;
    let next_status = status.unwrap_or(current.status);

    let updated = sqlx::query(
        "UPDATE contacts SET first_name = ?, last_name = ?, email = ?, email_normalized = ?, \
         identity_key = ?, status = ?, notes = ?, version = version + 1, updated_at = ? \
         WHERE id = ? AND version = ?",
    )
    .bind(&first_name)
    .bind(&last_name)
    .bind(&email)
    .bind(&email_normalized)
    .bind(&identity_key)
    .bind(next_status.as_str())
    .bind(&notes)
    .bind(now_ms)
    .bind(id)
    .bind(expected_version)
    .execute(&mut **tx)
    .await
    .map_err(StoreError::from)?;
    if updated.rows_affected() == 0 {
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM contacts WHERE id = ?)")
                .bind(id)
                .fetch_one(&mut **tx)
                .await
                .map_err(StoreError::from)?;
        if exists || missing_is_conflict {
            return Err(CoreError::Conflict(stale_message));
        }
        return Err(CoreError::NotFound {
            entity: "contact",
            id: id.to_string(),
        });
    }

    if email_changed {
        sqlx::query(
            "UPDATE contact_email_history SET valid_to = ? \
             WHERE contact_id = ? AND valid_to IS NULL",
        )
        .bind(now_ms)
        .bind(id)
        .execute(&mut **tx)
        .await
        .map_err(StoreError::from)?;
        sqlx::query(
            "INSERT INTO contact_email_history \
             (contact_id, email, email_normalized, valid_from, reason) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(id)
        .bind(&email)
        .bind(&email_normalized)
        .bind(now_ms)
        .bind(reason.as_str())
        .execute(&mut **tx)
        .await
        .map_err(StoreError::from)?;
    }

    let stored = Contact {
        id,
        first_name,
        last_name,
        email,
        email_normalized,
        identity_key,
        status: next_status,
        source: current.source,
        import_batch_id: current.import_batch_id,
        date_added: current.date_added,
        updated_at: now_ms,
        version: current.version + 1,
        notes,
        metadata: current.metadata.clone(),
    };
    Ok((current, stored))
}

fn create_reason(source: ContactSource) -> EmailChangeReason {
    match source {
        ContactSource::Import => EmailChangeReason::Import,
        _ => EmailChangeReason::Manual,
    }
}

fn contact_from_row(row: &SqliteRow) -> CoreResult<Contact> {
    Ok(Contact {
        id: row.try_get("id").map_err(StoreError::from)?,
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
        import_batch_id: row.try_get("import_batch_id").map_err(StoreError::from)?,
        date_added: row.try_get("date_added").map_err(StoreError::from)?,
        updated_at: row.try_get("updated_at").map_err(StoreError::from)?,
        version: row.try_get("version").map_err(StoreError::from)?,
        notes: row.try_get("notes").map_err(StoreError::from)?,
        metadata: crate::codec::decode_json(
            "contacts.metadata_json",
            &row.try_get::<String, _>("metadata_json")
                .map_err(StoreError::from)?,
        )?,
    })
}
