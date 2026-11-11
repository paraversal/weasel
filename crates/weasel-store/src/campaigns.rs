//! SQLite campaign repository. Every write runs in one `BEGIN IMMEDIATE` transaction and
//! records its own audit row.

use async_trait::async_trait;
use sqlx::{Executor, QueryBuilder, Row, Sqlite, sqlite::SqliteRow};
use std::collections::HashMap;
use weasel_core::{
    CoreError, CoreResult,
    audit::{AuditRecord, actions},
    campaign::{
        AudienceFilter, AudienceMember, Campaign, CampaignPatch, CampaignQuery, CampaignStats,
        NewCampaign, can_transition,
    },
    enums::CampaignStatus,
    ports::CampaignRepository,
    query::{Page, PageRequest, like_pattern},
};

use crate::{audit, codec::decode_enum, error::StoreError, pool::Store};

const CAMPAIGN_COLUMNS: &str =
    "id, name, description, subject, body, status, created_at, updated_at, version";

#[derive(Debug, Clone)]
pub struct SqliteCampaigns {
    store: Store,
}

impl SqliteCampaigns {
    pub fn new(store: Store) -> Self {
        Self { store }
    }

    async fn fetch_one<'e, E>(executor: E, id: i64) -> CoreResult<Campaign>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let row = sqlx::query(
            "SELECT id, name, description, subject, body, status, created_at, updated_at, version \
             FROM campaigns WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(executor)
        .await
        .map_err(StoreError::from)?
        .ok_or_else(|| CoreError::NotFound {
            entity: "campaign",
            id: id.to_string(),
        })?;
        campaign_from_row(&row)
    }
}

#[async_trait]
impl CampaignRepository for SqliteCampaigns {
    async fn list(&self, query: &CampaignQuery) -> CoreResult<Page<Campaign>> {
        let page = query.page.normalized();
        let search = like_pattern(query.search.as_deref().unwrap_or(""));

        let mut rows_query =
            QueryBuilder::<Sqlite>::new(format!("SELECT {CAMPAIGN_COLUMNS} FROM campaigns"));
        push_filters(&mut rows_query, query.status, search.as_deref());
        rows_query
            .push(" ORDER BY created_at DESC, id DESC LIMIT ")
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
            .map(campaign_from_row)
            .collect::<CoreResult<Vec<_>>>()?;

        let mut count_query = QueryBuilder::<Sqlite>::new("SELECT COUNT(*) FROM campaigns");
        push_filters(&mut count_query, query.status, search.as_deref());
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

    async fn get(&self, id: i64) -> CoreResult<Campaign> {
        Self::fetch_one(self.store.pool(), id).await
    }

    async fn create(&self, campaign: NewCampaign, now_ms: i64) -> CoreResult<Campaign> {
        let mut tx = self.store.begin_write().await?;
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO campaigns \
             (name, description, subject, body, status, created_at, updated_at, version) \
             VALUES (?, ?, ?, ?, 'draft', ?, ?, 1) RETURNING id",
        )
        .bind(&campaign.name)
        .bind(&campaign.description)
        .bind(&campaign.subject)
        .bind(&campaign.body)
        .bind(now_ms)
        .bind(now_ms)
        .fetch_one(&mut *tx)
        .await
        .map_err(StoreError::from)?;

        let stored = Campaign {
            id,
            name: campaign.name,
            description: campaign.description,
            subject: campaign.subject,
            body: campaign.body,
            status: CampaignStatus::Draft,
            created_at: now_ms,
            updated_at: now_ms,
            version: 1,
        };

        let mut event = AuditRecord::new(actions::CAMPAIGN_CREATE, "campaign");
        event.entity_id = Some(id.to_string());
        event.after = Some(serde_json::to_value(&stored).map_err(CoreError::internal)?);
        audit::record(&mut tx, &event, now_ms).await?;

        tx.commit().await.map_err(StoreError::from)?;
        Ok(stored)
    }

    async fn update(&self, id: i64, patch: CampaignPatch, now_ms: i64) -> CoreResult<Campaign> {
        let mut tx = self.store.begin_write().await?;
        let current = Self::fetch_one(&mut *tx, id).await?;

        let updated = Campaign {
            name: patch.name.unwrap_or_else(|| current.name.clone()),
            description: patch
                .description
                .unwrap_or_else(|| current.description.clone()),
            subject: patch.subject.unwrap_or_else(|| current.subject.clone()),
            body: patch.body.unwrap_or_else(|| current.body.clone()),
            updated_at: now_ms,
            version: current.version + 1,
            ..current.clone()
        };

        let result = sqlx::query(
            "UPDATE campaigns SET name = ?, description = ?, subject = ?, body = ?, \
             version = version + 1, updated_at = ? WHERE id = ? AND version = ?",
        )
        .bind(&updated.name)
        .bind(&updated.description)
        .bind(&updated.subject)
        .bind(&updated.body)
        .bind(now_ms)
        .bind(id)
        .bind(patch.version)
        .execute(&mut *tx)
        .await
        .map_err(StoreError::from)?;
        if result.rows_affected() == 0 {
            return Err(stale_version(&mut tx, id, patch.version).await);
        }

        let mut event = AuditRecord::new(actions::CAMPAIGN_UPDATE, "campaign");
        event.entity_id = Some(id.to_string());
        event.before = Some(serde_json::to_value(&current).map_err(CoreError::internal)?);
        event.after = Some(serde_json::to_value(&updated).map_err(CoreError::internal)?);
        audit::record(&mut tx, &event, now_ms).await?;

        tx.commit().await.map_err(StoreError::from)?;
        Ok(updated)
    }

    async fn set_status(
        &self,
        id: i64,
        expected_version: i64,
        status: CampaignStatus,
        now_ms: i64,
    ) -> CoreResult<Campaign> {
        let mut tx = self.store.begin_write().await?;
        let current = Self::fetch_one(&mut *tx, id).await?;
        if !can_transition(current.status, status) {
            return Err(CoreError::InvalidState(format!(
                "A campaign cannot move from {} to {}.",
                current.status.as_str(),
                status.as_str()
            )));
        }

        let result = sqlx::query(
            "UPDATE campaigns SET status = ?, version = version + 1, updated_at = ? \
             WHERE id = ? AND version = ?",
        )
        .bind(status.as_str())
        .bind(now_ms)
        .bind(id)
        .bind(expected_version)
        .execute(&mut *tx)
        .await
        .map_err(StoreError::from)?;
        if result.rows_affected() == 0 {
            return Err(stale_version(&mut tx, id, expected_version).await);
        }

        let stored = Campaign {
            status,
            updated_at: now_ms,
            version: current.version + 1,
            ..current.clone()
        };
        let mut event = AuditRecord::new(actions::CAMPAIGN_STATUS, "campaign");
        event.entity_id = Some(id.to_string());
        event.before = Some(serde_json::to_value(&current).map_err(CoreError::internal)?);
        event.after = Some(serde_json::to_value(&stored).map_err(CoreError::internal)?);
        audit::record(&mut tx, &event, now_ms).await?;

        tx.commit().await.map_err(StoreError::from)?;
        Ok(stored)
    }

    async fn delete(&self, id: i64, expected_version: i64, now_ms: i64) -> CoreResult<()> {
        let mut tx = self.store.begin_write().await?;
        let current = Self::fetch_one(&mut *tx, id).await?;

        // Invariant 7: send history keeps a campaign alive.
        let has_batches: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM send_batches WHERE campaign_id = ?)")
                .bind(id)
                .fetch_one(&mut *tx)
                .await
                .map_err(StoreError::from)?;
        if has_batches {
            return Err(CoreError::InvalidState(
                "Campaign has send history; archive it instead".to_owned(),
            ));
        }

        let result = sqlx::query("DELETE FROM campaigns WHERE id = ? AND version = ?")
            .bind(id)
            .bind(expected_version)
            .execute(&mut *tx)
            .await
            .map_err(StoreError::from)?;
        if result.rows_affected() == 0 {
            return Err(stale_version(&mut tx, id, expected_version).await);
        }

        let mut event = AuditRecord::new(actions::CAMPAIGN_DELETE, "campaign");
        event.entity_id = Some(id.to_string());
        event.before = Some(serde_json::to_value(&current).map_err(CoreError::internal)?);
        audit::record(&mut tx, &event, now_ms).await?;

        tx.commit().await.map_err(StoreError::from)?;
        Ok(())
    }

    async fn stats(&self, id: i64) -> CoreResult<CampaignStats> {
        Self::fetch_one(self.store.pool(), id).await?;
        Ok(self.stats_for(&[id]).await?.remove(&id).unwrap_or_default())
    }

    async fn stats_for(&self, ids: &[i64]) -> CoreResult<HashMap<i64, CampaignStats>> {
        if ids.is_empty() {
            return Ok(HashMap::new());
        }

        let totals = sqlx::query(
            "SELECT status, COUNT(*) AS n FROM contacts \
             WHERE status IN ('active', 'removed') GROUP BY status",
        )
        .fetch_all(self.store.pool())
        .await
        .map_err(StoreError::from)?;
        let mut active = 0;
        let mut removed = 0;
        for row in &totals {
            let status: String = row.try_get("status").map_err(StoreError::from)?;
            let n = count(row, "n")?;
            match status.as_str() {
                "active" => active = n,
                "removed" => removed = n,
                _ => {}
            }
        }

        let mut stats: HashMap<i64, CampaignStats> = ids
            .iter()
            .map(|id| {
                (
                    *id,
                    CampaignStats {
                        unsent: active,
                        removed,
                        total: active,
                        ..CampaignStats::default()
                    },
                )
            })
            .collect();

        let mut batches = QueryBuilder::<Sqlite>::new(
            "SELECT b.campaign_id AS campaign_id, COUNT(r.id) AS sent, \
             MAX(b.created_at) AS last_batch_at \
             FROM send_batches b LEFT JOIN send_records r ON r.batch_id = b.id \
             WHERE b.campaign_id IN (",
        );
        push_ids(&mut batches, ids);
        batches.push(") GROUP BY b.campaign_id");
        let batches = batches
            .build()
            .fetch_all(self.store.pool())
            .await
            .map_err(StoreError::from)?;
        for row in &batches {
            let campaign_id: i64 = row.try_get("campaign_id").map_err(StoreError::from)?;
            if let Some(entry) = stats.get_mut(&campaign_id) {
                entry.sent = count(row, "sent")?;
                entry.last_batch_at = row.try_get("last_batch_at").map_err(StoreError::from)?;
            }
        }

        let mut sent_contacts = QueryBuilder::<Sqlite>::new(
            "SELECT b.campaign_id AS campaign_id, c.status AS status, \
             COUNT(DISTINCT r.contact_id) AS n \
             FROM send_records r \
             JOIN send_batches b ON b.id = r.batch_id \
             JOIN contacts c ON c.id = r.contact_id \
             WHERE b.campaign_id IN (",
        );
        push_ids(&mut sent_contacts, ids);
        sent_contacts.push(") GROUP BY b.campaign_id, c.status");
        let sent_contacts = sent_contacts
            .build()
            .fetch_all(self.store.pool())
            .await
            .map_err(StoreError::from)?;
        for row in &sent_contacts {
            let campaign_id: i64 = row.try_get("campaign_id").map_err(StoreError::from)?;
            let status: String = row.try_get("status").map_err(StoreError::from)?;
            let n = count(row, "n")?;
            if let Some(entry) = stats.get_mut(&campaign_id) {
                match status.as_str() {
                    "active" => entry.unsent = entry.unsent.saturating_sub(n),
                    "removed" => entry.removed = entry.removed.saturating_sub(n),
                    _ => {}
                }
            }
        }

        for entry in stats.values_mut() {
            entry.total = entry.sent + entry.unsent;
        }
        Ok(stats)
    }

    async fn audience(
        &self,
        id: i64,
        filter: AudienceFilter,
        search: Option<&str>,
        page: &PageRequest,
    ) -> CoreResult<Page<AudienceMember>> {
        Self::fetch_one(self.store.pool(), id).await?;
        let page = page.normalized();
        let search = like_pattern(search.unwrap_or(""));

        let mut rows_query = QueryBuilder::<Sqlite>::new(
            "SELECT contact_id, first_name, last_name, email, contact_status, batch_id, \
             sent_at, sort_id FROM (",
        );
        push_audience_arms(&mut rows_query, id, filter);
        rows_query.push(") m");
        push_audience_search(&mut rows_query, search.as_deref());
        rows_query
            .push(
                " ORDER BY last_name COLLATE NOCASE ASC, first_name COLLATE NOCASE ASC, \
                   sort_id ASC LIMIT ",
            )
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
            .map(audience_from_row)
            .collect::<CoreResult<Vec<_>>>()?;

        let mut count_query = QueryBuilder::<Sqlite>::new("SELECT COUNT(*) FROM (");
        push_audience_arms(&mut count_query, id, filter);
        count_query.push(") m");
        push_audience_search(&mut count_query, search.as_deref());
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
}

fn push_filters(
    query: &mut QueryBuilder<Sqlite>,
    status: Option<CampaignStatus>,
    search: Option<&str>,
) {
    query.push(" WHERE 1 = 1");
    if let Some(status) = status {
        query.push(" AND status = ").push_bind(status.as_str());
    }
    if let Some(pattern) = search {
        query
            .push(" AND name LIKE ")
            .push_bind(pattern.to_owned())
            .push(" ESCAPE '\\'");
    }
}

fn push_audience_arms(query: &mut QueryBuilder<Sqlite>, campaign_id: i64, filter: AudienceFilter) {
    if matches!(filter, AudienceFilter::Sent | AudienceFilter::All) {
        query.push(
            "SELECT r.contact_id AS contact_id, r.first_name AS first_name, \
             r.last_name AS last_name, r.email AS email, c.status AS contact_status, \
             r.batch_id AS batch_id, b.created_at AS sent_at, r.id AS sort_id \
             FROM send_records r \
             JOIN send_batches b ON b.id = r.batch_id \
             LEFT JOIN contacts c ON c.id = r.contact_id \
             WHERE b.campaign_id = ",
        );
        query.push_bind(campaign_id);
    }
    if filter == AudienceFilter::All {
        query.push(" UNION ALL ");
    }
    if matches!(filter, AudienceFilter::Unsent | AudienceFilter::All) {
        query.push(
            "SELECT c.id AS contact_id, c.first_name AS first_name, \
             c.last_name AS last_name, c.email AS email, c.status AS contact_status, \
             NULL AS batch_id, NULL AS sent_at, c.id AS sort_id \
             FROM contacts c \
             WHERE c.status = 'active' AND NOT EXISTS \
             (SELECT 1 FROM send_records r JOIN send_batches b ON b.id = r.batch_id \
              WHERE b.campaign_id = ",
        );
        query.push_bind(campaign_id);
        query.push(" AND r.contact_id = c.id)");
    }
}

fn push_audience_search(query: &mut QueryBuilder<Sqlite>, search: Option<&str>) {
    if let Some(pattern) = search {
        query
            .push(" WHERE (first_name LIKE ")
            .push_bind(pattern.to_owned())
            .push(" ESCAPE '\\' OR last_name LIKE ")
            .push_bind(pattern.to_owned())
            .push(" ESCAPE '\\' OR email LIKE ")
            .push_bind(pattern.to_owned())
            .push(" ESCAPE '\\')");
    }
}

async fn stale_version(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    id: i64,
    expected_version: i64,
) -> CoreError {
    let exists: Option<i64> = sqlx::query_scalar("SELECT 1 FROM campaigns WHERE id = ?")
        .bind(id)
        .fetch_optional(&mut **tx)
        .await
        .ok()
        .flatten();
    if exists.is_none() {
        CoreError::NotFound {
            entity: "campaign",
            id: id.to_string(),
        }
    } else {
        CoreError::Conflict(format!(
            "This campaign was changed elsewhere. Reload and try again (expected version \
             {expected_version})."
        ))
    }
}

fn push_ids(query: &mut QueryBuilder<Sqlite>, ids: &[i64]) {
    for (index, id) in ids.iter().enumerate() {
        if index > 0 {
            query.push(", ");
        }
        query.push_bind(*id);
    }
}

fn count(row: &SqliteRow, column: &str) -> CoreResult<u64> {
    Ok(u64::try_from(row.try_get::<i64, _>(column).map_err(StoreError::from)?).unwrap_or_default())
}

fn audience_from_row(row: &SqliteRow) -> CoreResult<AudienceMember> {
    let contact_status = row
        .try_get::<Option<String>, _>("contact_status")
        .map_err(StoreError::from)?
        .map(|raw| decode_enum("contacts.status", raw))
        .transpose()?;
    Ok(AudienceMember {
        contact_id: row.try_get("contact_id").map_err(StoreError::from)?,
        first_name: row.try_get("first_name").map_err(StoreError::from)?,
        last_name: row.try_get("last_name").map_err(StoreError::from)?,
        email: row.try_get("email").map_err(StoreError::from)?,
        contact_status,
        batch_id: row.try_get("batch_id").map_err(StoreError::from)?,
        sent_at: row.try_get("sent_at").map_err(StoreError::from)?,
    })
}

fn campaign_from_row(row: &SqliteRow) -> CoreResult<Campaign> {
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
