//! SQLite invalid-address flags and rules, plus the shared "on hold" SQL predicate that
//! the contact, campaign and send queries use.

use std::collections::HashMap;

use async_trait::async_trait;
use sqlx::{Row, Sqlite, Transaction};
use weasel_core::{
    CoreError, CoreResult,
    audit::{AuditRecord, actions},
    email::EmailAddress,
    email_check::{
        EmailFlag, EmailHold, EmailVerdict, FlagReason, InvalidRule, PREVIEW_EXAMPLES, RulePreview,
    },
    enums::SendRecordStatus,
    ports::EmailCheckRepository,
    send::SendRecord,
};

use crate::{audit, codec::decode_enum, error::StoreError, pool::Store};

/// SQL that is true when the normalized address in `column` is on hold: flagged
/// invalid, or matched by a rule and not overridden as valid.
pub(crate) fn on_hold_sql(column: &str) -> String {
    format!(
        "(EXISTS (SELECT 1 FROM email_flags ef WHERE ef.email_normalized = {column} \
           AND ef.verdict = 'invalid') \
         OR (NOT EXISTS (SELECT 1 FROM email_flags ef WHERE ef.email_normalized = {column}) \
           AND EXISTS (SELECT 1 FROM invalid_email_rules er WHERE {column} GLOB er.pattern)))"
    )
}

/// A send record counts as received unless it bounced.
pub(crate) const DELIVERED_SQL: &str = "r.status <> 'failed'";

#[derive(Debug, Clone)]
pub struct SqliteEmailChecks {
    store: Store,
}

impl SqliteEmailChecks {
    pub fn new(store: Store) -> Self {
        Self { store }
    }
}

#[async_trait]
impl EmailCheckRepository for SqliteEmailChecks {
    async fn rules(&self) -> CoreResult<Vec<InvalidRule>> {
        let rows = sqlx::query(
            "SELECT er.id, er.pattern, er.created_at, \
             (SELECT COUNT(*) FROM contacts c WHERE c.status = 'active' \
              AND c.email_normalized GLOB er.pattern) AS matches \
             FROM invalid_email_rules er ORDER BY er.pattern",
        )
        .fetch_all(self.store.pool())
        .await
        .map_err(StoreError::from)?;
        rows.iter()
            .map(|row| {
                Ok(InvalidRule {
                    id: row.try_get("id").map_err(StoreError::from)?,
                    pattern: row.try_get("pattern").map_err(StoreError::from)?,
                    created_at: row.try_get("created_at").map_err(StoreError::from)?,
                    matches: count(row, "matches")?,
                })
            })
            .collect()
    }

    async fn add_rule(&self, pattern: &str, now_ms: i64) -> CoreResult<InvalidRule> {
        let mut tx = self.store.begin_write().await?;
        let exists: Option<i64> =
            sqlx::query_scalar("SELECT id FROM invalid_email_rules WHERE pattern = ?")
                .bind(pattern)
                .fetch_optional(&mut *tx)
                .await
                .map_err(StoreError::from)?;
        if exists.is_some() {
            return Err(CoreError::Conflict("This rule already exists".to_owned()));
        }
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO invalid_email_rules (pattern, created_at) VALUES (?, ?) RETURNING id",
        )
        .bind(pattern)
        .bind(now_ms)
        .fetch_one(&mut *tx)
        .await
        .map_err(StoreError::from)?;
        let matches: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM contacts WHERE status = 'active' AND email_normalized GLOB ?",
        )
        .bind(pattern)
        .fetch_one(&mut *tx)
        .await
        .map_err(StoreError::from)?;

        let mut event = AuditRecord::new(actions::EMAIL_RULE_ADD, "email_rule");
        event.entity_id = Some(id.to_string());
        event.after = Some(serde_json::json!({ "pattern": pattern }));
        audit::record(&mut tx, &event, now_ms).await?;
        tx.commit().await.map_err(StoreError::from)?;

        Ok(InvalidRule {
            id,
            pattern: pattern.to_owned(),
            created_at: now_ms,
            matches: u64::try_from(matches).unwrap_or_default(),
        })
    }

    async fn delete_rule(&self, id: i64, now_ms: i64) -> CoreResult<()> {
        let mut tx = self.store.begin_write().await?;
        let pattern: Option<String> =
            sqlx::query_scalar("DELETE FROM invalid_email_rules WHERE id = ? RETURNING pattern")
                .bind(id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(StoreError::from)?;
        let Some(pattern) = pattern else {
            return Err(CoreError::NotFound {
                entity: "email rule",
                id: id.to_string(),
            });
        };
        let mut event = AuditRecord::new(actions::EMAIL_RULE_DELETE, "email_rule");
        event.entity_id = Some(id.to_string());
        event.before = Some(serde_json::json!({ "pattern": pattern }));
        audit::record(&mut tx, &event, now_ms).await?;
        tx.commit().await.map_err(StoreError::from)?;
        Ok(())
    }

    async fn preview_rule(&self, pattern: &str) -> CoreResult<RulePreview> {
        let matches: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM contacts WHERE status = 'active' AND email_normalized GLOB ?",
        )
        .bind(pattern)
        .fetch_one(self.store.pool())
        .await
        .map_err(StoreError::from)?;
        let examples: Vec<String> = sqlx::query_scalar(
            "SELECT email FROM contacts WHERE status = 'active' AND email_normalized GLOB ? \
             ORDER BY email_normalized LIMIT ?",
        )
        .bind(pattern)
        .bind(i64::try_from(PREVIEW_EXAMPLES).unwrap_or(5))
        .fetch_all(self.store.pool())
        .await
        .map_err(StoreError::from)?;
        Ok(RulePreview {
            pattern: pattern.to_owned(),
            matches: u64::try_from(matches).unwrap_or_default(),
            examples,
        })
    }

    async fn flags(&self) -> CoreResult<Vec<EmailFlag>> {
        let rows = sqlx::query(
            "SELECT ef.email, ef.email_normalized, ef.verdict, ef.reason, ef.created_at, \
             (SELECT COUNT(*) FROM contacts c WHERE c.status = 'active' \
              AND c.email_normalized = ef.email_normalized) AS contacts \
             FROM email_flags ef ORDER BY ef.created_at DESC, ef.email_normalized",
        )
        .fetch_all(self.store.pool())
        .await
        .map_err(StoreError::from)?;
        rows.iter()
            .map(|row| {
                Ok(EmailFlag {
                    email: row.try_get("email").map_err(StoreError::from)?,
                    email_normalized: row.try_get("email_normalized").map_err(StoreError::from)?,
                    verdict: decode_enum(
                        "email_flags.verdict",
                        row.try_get("verdict").map_err(StoreError::from)?,
                    )?,
                    reason: decode_enum(
                        "email_flags.reason",
                        row.try_get("reason").map_err(StoreError::from)?,
                    )?,
                    created_at: row.try_get("created_at").map_err(StoreError::from)?,
                    contacts: count(row, "contacts")?,
                })
            })
            .collect()
    }

    async fn set_flag(
        &self,
        email: &EmailAddress,
        verdict: EmailVerdict,
        reason: FlagReason,
        now_ms: i64,
    ) -> CoreResult<()> {
        let mut tx = self.store.begin_write().await?;
        write_flag(&mut tx, email, verdict, reason, now_ms).await?;
        tx.commit().await.map_err(StoreError::from)?;
        Ok(())
    }

    async fn mark_valid(&self, email: &EmailAddress, now_ms: i64) -> CoreResult<()> {
        let mut tx = self.store.begin_write().await?;
        let ruled: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM invalid_email_rules WHERE ? GLOB pattern)",
        )
        .bind(&email.normalized)
        .fetch_one(&mut *tx)
        .await
        .map_err(StoreError::from)?;
        if ruled {
            write_flag(
                &mut tx,
                email,
                EmailVerdict::Valid,
                FlagReason::Manual,
                now_ms,
            )
            .await?;
        } else {
            remove_flag(&mut tx, email, now_ms).await?;
        }
        tx.commit().await.map_err(StoreError::from)?;
        Ok(())
    }

    async fn clear_flag(&self, email: &EmailAddress, now_ms: i64) -> CoreResult<()> {
        let mut tx = self.store.begin_write().await?;
        remove_flag(&mut tx, email, now_ms).await?;
        tx.commit().await.map_err(StoreError::from)?;
        Ok(())
    }

    async fn holds(&self, normalized: &[String]) -> CoreResult<HashMap<String, EmailHold>> {
        let list = serde_json::to_string(normalized).map_err(CoreError::internal)?;
        let rows = sqlx::query(
            "SELECT DISTINCT j.value AS address, \
             (SELECT ef.verdict FROM email_flags ef WHERE ef.email_normalized = j.value) \
               AS verdict, \
             (SELECT ef.reason FROM email_flags ef WHERE ef.email_normalized = j.value) \
               AS reason, \
             (SELECT er.pattern FROM invalid_email_rules er WHERE j.value GLOB er.pattern \
              ORDER BY er.id LIMIT 1) AS pattern \
             FROM json_each(?) j",
        )
        .bind(list)
        .fetch_all(self.store.pool())
        .await
        .map_err(StoreError::from)?;

        let mut holds = HashMap::new();
        for row in &rows {
            let address: String = row.try_get("address").map_err(StoreError::from)?;
            let verdict: Option<String> = row.try_get("verdict").map_err(StoreError::from)?;
            let reason: Option<String> = row.try_get("reason").map_err(StoreError::from)?;
            let pattern: Option<String> = row.try_get("pattern").map_err(StoreError::from)?;
            let hold = match (verdict.as_deref(), reason.as_deref(), pattern) {
                (Some("invalid"), Some("bounced"), _) => Some(EmailHold::Bounced),
                (Some("invalid"), _, _) => Some(EmailHold::Manual),
                (Some(_), _, _) => None,
                (None, _, Some(pattern)) => Some(EmailHold::Rule { pattern }),
                (None, _, None) => None,
            };
            if let Some(hold) = hold {
                holds.insert(address, hold);
            }
        }
        Ok(holds)
    }

    async fn set_bounced(
        &self,
        record_id: i64,
        bounced: bool,
        now_ms: i64,
    ) -> CoreResult<SendRecord> {
        let mut tx = self.store.begin_write().await?;
        let row = sqlx::query(
            "SELECT id, batch_id, contact_id, email, email_normalized, first_name, last_name, \
             status, created_at FROM send_records WHERE id = ?",
        )
        .bind(record_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(StoreError::from)?
        .ok_or_else(|| CoreError::NotFound {
            entity: "send record",
            id: record_id.to_string(),
        })?;
        let email = EmailAddress {
            display: row.try_get("email").map_err(StoreError::from)?,
            normalized: row.try_get("email_normalized").map_err(StoreError::from)?,
        };
        let current: SendRecordStatus = decode_enum(
            "send_records.status",
            row.try_get("status").map_err(StoreError::from)?,
        )?;
        let next = if bounced {
            SendRecordStatus::Failed
        } else {
            SendRecordStatus::Copied
        };

        let mut record = SendRecord {
            id: record_id,
            batch_id: row.try_get("batch_id").map_err(StoreError::from)?,
            contact_id: row.try_get("contact_id").map_err(StoreError::from)?,
            email: email.display.clone(),
            first_name: row.try_get("first_name").map_err(StoreError::from)?,
            last_name: row.try_get("last_name").map_err(StoreError::from)?,
            status: current,
            created_at: row.try_get("created_at").map_err(StoreError::from)?,
        };
        if (current == SendRecordStatus::Failed) == bounced {
            return Ok(record);
        }

        sqlx::query("UPDATE send_records SET status = ? WHERE id = ?")
            .bind(next.as_str())
            .bind(record_id)
            .execute(&mut *tx)
            .await
            .map_err(StoreError::from)?;
        if bounced {
            write_flag(
                &mut tx,
                &email,
                EmailVerdict::Invalid,
                FlagReason::Bounced,
                now_ms,
            )
            .await?;
        } else {
            // Undo only the flag this bounce created; a manual verdict stays.
            let reason: Option<String> = sqlx::query_scalar(
                "SELECT reason FROM email_flags WHERE email_normalized = ? AND verdict = 'invalid'",
            )
            .bind(&email.normalized)
            .fetch_optional(&mut *tx)
            .await
            .map_err(StoreError::from)?;
            if reason.as_deref() == Some(FlagReason::Bounced.as_str()) {
                remove_flag(&mut tx, &email, now_ms).await?;
            }
        }

        let mut event = AuditRecord::new(actions::SEND_RECORD_BOUNCE, "send_record");
        event.entity_id = Some(record_id.to_string());
        event.before = Some(serde_json::json!({ "status": current.as_str() }));
        event.after = Some(serde_json::json!({ "status": next.as_str() }));
        event.metadata = serde_json::json!({ "batchId": record.batch_id });
        audit::record(&mut tx, &event, now_ms).await?;
        tx.commit().await.map_err(StoreError::from)?;

        record.status = next;
        Ok(record)
    }
}

async fn write_flag(
    tx: &mut Transaction<'_, Sqlite>,
    email: &EmailAddress,
    verdict: EmailVerdict,
    reason: FlagReason,
    now_ms: i64,
) -> CoreResult<()> {
    sqlx::query(
        "INSERT INTO email_flags (email_normalized, email, verdict, reason, created_at) \
         VALUES (?, ?, ?, ?, ?) \
         ON CONFLICT (email_normalized) DO UPDATE SET email = excluded.email, \
         verdict = excluded.verdict, reason = excluded.reason, created_at = excluded.created_at",
    )
    .bind(&email.normalized)
    .bind(&email.display)
    .bind(verdict.as_str())
    .bind(reason.as_str())
    .bind(now_ms)
    .execute(&mut **tx)
    .await
    .map_err(StoreError::from)?;
    let mut event = AuditRecord::new(actions::EMAIL_FLAG, "email");
    event.entity_id = Some(email.normalized.clone());
    event.after = Some(serde_json::json!({
        "verdict": verdict.as_str(),
        "reason": reason.as_str(),
    }));
    audit::record(tx, &event, now_ms).await
}

async fn remove_flag(
    tx: &mut Transaction<'_, Sqlite>,
    email: &EmailAddress,
    now_ms: i64,
) -> CoreResult<()> {
    let removed = sqlx::query("DELETE FROM email_flags WHERE email_normalized = ?")
        .bind(&email.normalized)
        .execute(&mut **tx)
        .await
        .map_err(StoreError::from)?;
    if removed.rows_affected() > 0 {
        let mut event = AuditRecord::new(actions::EMAIL_UNFLAG, "email");
        event.entity_id = Some(email.normalized.clone());
        audit::record(tx, &event, now_ms).await?;
    }
    Ok(())
}

fn count(row: &sqlx::sqlite::SqliteRow, column: &str) -> CoreResult<u64> {
    let n: i64 = row.try_get(column).map_err(StoreError::from)?;
    Ok(u64::try_from(n).unwrap_or_default())
}
