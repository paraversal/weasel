#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::Path;

use sqlx::{
    Row,
    migrate::Migrator,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
use weasel_store::{OpenOptions, get_meta, open, open_with};

/// A database that only has `0001_app.sql` must be backed up, then upgraded in place.
#[tokio::test]
async fn v1_database_is_backed_up_before_upgrade() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("weasel.sqlite3");
    let backups_dir = dir.path().join("backups");
    let v1_dir = dir.path().join("v1");
    std::fs::create_dir(&v1_dir).unwrap();
    let v1_sql = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../migrations/0001_app.sql");
    std::fs::copy(v1_sql, v1_dir.join("0001_app.sql")).unwrap();
    let v1 = Migrator::new(v1_dir.as_path()).await.unwrap();

    let (store, report) = open_with(
        OpenOptions {
            db_path: &db_path,
            backups_dir: &backups_dir,
        },
        &v1,
    )
    .await
    .unwrap();
    assert_eq!(report.schema_version, 1);
    store.close().await;

    let (store, report) = open(OpenOptions {
        db_path: &db_path,
        backups_dir: &backups_dir,
    })
    .await
    .unwrap();
    assert_eq!(report.schema_version, 3);
    let backup = report.backup.expect("upgrade must take a backup");
    assert!(backup.is_file(), "backup missing at {}", backup.display());

    // The backup is the pre-upgrade database: v1 and no domain tables.
    let opts = SqliteConnectOptions::new()
        .filename(&backup)
        .read_only(true)
        .create_if_missing(false);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .unwrap();
    let version: i64 = sqlx::query_scalar("SELECT COALESCE(MAX(version), 0) FROM _sqlx_migrations")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(version, 1);
    let row = sqlx::query(
        "SELECT COUNT(*) AS n FROM sqlite_master WHERE type = 'table' AND name = 'contacts'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.get::<i64, _>("n"), 0);
    pool.close().await;

    let recorded = get_meta(&store, "last_backup_path").await.unwrap();
    assert_eq!(recorded.as_deref(), Some(backup.to_string_lossy().as_ref()));
    weasel_testkit::assert::assert_integrity(&store).await;
}
