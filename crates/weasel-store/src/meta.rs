use crate::{error::StoreError, pool::Store};

/// Read an instance-metadata value.
pub async fn get_meta(store: &Store, key: &str) -> Result<Option<String>, StoreError> {
    let value = sqlx::query_scalar::<_, String>("SELECT value FROM app_meta WHERE key = ?")
        .bind(key)
        .fetch_optional(store.pool())
        .await?;
    Ok(value)
}

/// Insert or replace an instance-metadata value.
pub async fn set_meta(store: &Store, key: &str, value: &str) -> Result<(), StoreError> {
    sqlx::query(
        "INSERT INTO app_meta (key, value) VALUES (?, ?) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
    )
    .bind(key)
    .bind(value)
    .execute(store.pool())
    .await?;
    Ok(())
}
