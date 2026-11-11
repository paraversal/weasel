use sqlx::{Sqlite, Transaction};

use crate::{error::StoreError, pool::Store};

impl Store {
    /// Start a write transaction with `BEGIN IMMEDIATE`.
    ///
    /// `pool.begin()` is `BEGIN DEFERRED`, which takes the write lock only at the first
    /// write. A read-then-write transaction can then fail with `SQLITE_BUSY` when it
    /// upgrades while another connection holds the lock. Every write path uses this.
    pub async fn begin_write(&self) -> Result<Transaction<'static, Sqlite>, StoreError> {
        self.pool()
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(StoreError::from)
    }

    /// Close the pool. Consumes the handle so no connection can outlive the call.
    pub async fn close(self) {
        self.pool().close().await;
    }
}
