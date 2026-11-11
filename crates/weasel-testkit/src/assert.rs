//! Database invariant assertions for tests.
//!
//! These helpers exist to fail a test; a broken database is the failure.
#![allow(clippy::expect_used)]

use weasel_store::Store;

/// Panic if any row violates a foreign key.
pub async fn assert_fk_clean(store: &Store) {
    let rows = sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(store.pool())
        .await
        .expect("PRAGMA foreign_key_check");
    assert!(
        rows.is_empty(),
        "foreign key violations: {} rows",
        rows.len()
    );
}

/// Panic unless SQLite reports the database as internally consistent.
pub async fn assert_integrity(store: &Store) {
    let verdict: String = sqlx::query_scalar("PRAGMA integrity_check")
        .fetch_one(store.pool())
        .await
        .expect("PRAGMA integrity_check");
    assert_eq!(verdict, "ok", "integrity_check: {verdict}");
}
