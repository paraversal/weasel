use async_trait::async_trait;
use serde_json::Value;
use weasel_core::{CoreError, CoreResult, ports::SettingsRepository};

use crate::{error::StoreError, pool::Store};

/// SQLite-backed [`SettingsRepository`]. Every `save` runs in one transaction.
#[derive(Debug, Clone)]
pub struct SqliteSettings {
    store: Store,
}

impl SqliteSettings {
    pub fn new(store: Store) -> Self {
        Self { store }
    }
}

#[async_trait]
impl SettingsRepository for SqliteSettings {
    async fn load(&self) -> CoreResult<Vec<(String, Value)>> {
        let rows = sqlx::query_as::<_, (String, String)>(
            "SELECT key, value_json FROM settings ORDER BY key",
        )
        .fetch_all(self.store.pool())
        .await
        .map_err(StoreError::from)?;

        rows.into_iter()
            .map(|(key, value)| {
                serde_json::from_str(&value)
                    .map(|value| (key.clone(), value))
                    .map_err(CoreError::internal)
            })
            .collect()
    }

    async fn save(&self, entries: Vec<(String, Value)>, now_ms: i64) -> CoreResult<()> {
        let mut tx = self.store.pool().begin().await.map_err(StoreError::from)?;
        for (key, value) in entries {
            let value_json = serde_json::to_string(&value).map_err(CoreError::internal)?;
            sqlx::query(
                "INSERT INTO settings (key, value_json, updated_at) VALUES (?, ?, ?) \
                 ON CONFLICT(key) DO UPDATE SET \
                 value_json = excluded.value_json, updated_at = excluded.updated_at",
            )
            .bind(key)
            .bind(value_json)
            .bind(now_ms)
            .execute(&mut *tx)
            .await
            .map_err(StoreError::from)?;
        }
        tx.commit().await.map_err(StoreError::from)?;
        Ok(())
    }
}
