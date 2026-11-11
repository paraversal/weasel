#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Pool setup, pragmas, backup-before-migrate and newer-schema refusal.

use std::path::{Path, PathBuf};

use sqlx::{
    SqlitePool,
    migrate::Migrator,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
use tempfile::TempDir;
use weasel_store::{OpenOptions, StoreError, get_meta, open, open_with};

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

async fn migrator(name: &str) -> Migrator {
    Migrator::new(fixtures().join(name)).await.unwrap()
}

fn opts(dir: &Path) -> (PathBuf, PathBuf) {
    (dir.join("weasel.sqlite3"), dir.join("backups"))
}

async fn open_file(path: &Path) -> SqlitePool {
    let connect = SqliteConnectOptions::new().filename(path).read_only(true);
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(connect)
        .await
        .unwrap()
}

#[tokio::test]
async fn fresh_open_creates_schema_and_metadata() {
    let dir = TempDir::new().unwrap();
    let (db_path, backups_dir) = opts(dir.path());

    let (store, report) = open(OpenOptions {
        db_path: &db_path,
        backups_dir: &backups_dir,
    })
    .await
    .unwrap();

    assert!(db_path.exists());
    assert!(report.created);
    assert!(report.backup.is_none());
    assert_eq!(report.schema_version, 1);
    assert_eq!(store.schema_version().await.unwrap(), 1);
    store.ping().await.unwrap();

    let foreign_keys: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(foreign_keys, 1);

    let journal_mode: String = sqlx::query_scalar("PRAGMA journal_mode")
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(journal_mode.to_ascii_lowercase(), "wal");

    let created_at = get_meta(&store, "created_at").await.unwrap().unwrap();
    assert!(created_at.parse::<i64>().unwrap() > 0);

    let instance_id = get_meta(&store, "instance_id").await.unwrap().unwrap();
    assert_eq!(instance_id.len(), 32);
    assert!(instance_id.chars().all(|c| c.is_ascii_hexdigit()));
}

#[tokio::test]
async fn reopen_applies_nothing_and_takes_no_backup() {
    let dir = TempDir::new().unwrap();
    let (db_path, backups_dir) = opts(dir.path());

    {
        let (store, _) = open(OpenOptions {
            db_path: &db_path,
            backups_dir: &backups_dir,
        })
        .await
        .unwrap();
        let instance_id = get_meta(&store, "instance_id").await.unwrap();
        assert!(instance_id.is_some());
    }

    let (store, report) = open(OpenOptions {
        db_path: &db_path,
        backups_dir: &backups_dir,
    })
    .await
    .unwrap();
    assert!(!report.created);
    assert!(report.backup.is_none());
    assert_eq!(report.schema_version, 1);
    assert!(!backups_dir.exists() || std::fs::read_dir(&backups_dir).unwrap().next().is_none());
    assert!(
        get_meta(&store, "last_backup_path")
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn pending_migration_backs_up_old_schema_first() {
    let dir = TempDir::new().unwrap();
    let (db_path, backups_dir) = opts(dir.path());
    let v1 = migrator("migrations_v1").await;
    let v2 = migrator("migrations_v2").await;

    {
        let (store, report) = open_with(
            OpenOptions {
                db_path: &db_path,
                backups_dir: &backups_dir,
            },
            &v1,
        )
        .await
        .unwrap();
        assert!(report.created);
        assert_eq!(report.schema_version, 1);
        sqlx::query("INSERT INTO probe (id, note) VALUES (1, 'before')")
            .execute(store.pool())
            .await
            .unwrap();
    }

    let (store, report) = open_with(
        OpenOptions {
            db_path: &db_path,
            backups_dir: &backups_dir,
        },
        &v2,
    )
    .await
    .unwrap();

    assert!(!report.created);
    assert_eq!(report.schema_version, 2);
    let backup = report.backup.expect("backup before migration");
    assert!(backup.exists());
    assert!(
        backup
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("weasel.sqlite3.pre-v2-"),
        "unexpected backup name: {}",
        backup.display()
    );

    let recorded = get_meta(&store, "last_backup_path").await.unwrap();
    assert_eq!(recorded.as_deref(), Some(backup.to_string_lossy().as_ref()));

    let extra: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM pragma_table_info('probe') WHERE name = 'extra'")
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(extra, 1);

    // The backup is the pre-migration schema and holds the data written there.
    let backup_pool = open_file(&backup).await;
    let backup_version: i64 = sqlx::query_scalar("SELECT MAX(version) FROM _sqlx_migrations")
        .fetch_one(&backup_pool)
        .await
        .unwrap();
    assert_eq!(backup_version, 1);
    let note: String = sqlx::query_scalar("SELECT note FROM probe WHERE id = 1")
        .fetch_one(&backup_pool)
        .await
        .unwrap();
    assert_eq!(note, "before");
    let has_extra: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'probe'",
    )
    .fetch_one(&backup_pool)
    .await
    .unwrap();
    assert_eq!(has_extra, 1);
}

#[tokio::test]
async fn refuses_database_from_a_newer_app() {
    let dir = TempDir::new().unwrap();
    let (db_path, backups_dir) = opts(dir.path());
    let v1 = migrator("migrations_v1").await;
    let v2 = migrator("migrations_v2").await;

    open_with(
        OpenOptions {
            db_path: &db_path,
            backups_dir: &backups_dir,
        },
        &v2,
    )
    .await
    .unwrap();

    let err = open_with(
        OpenOptions {
            db_path: &db_path,
            backups_dir: &backups_dir,
        },
        &v1,
    )
    .await
    .unwrap_err();
    match err {
        StoreError::NewerSchema { db, app } => {
            assert_eq!((db, app), (2, 1));
        }
        other => panic!("expected NewerSchema, got {other:?}"),
    }

    let backups = std::fs::read_dir(&backups_dir)
        .map(|d| d.count())
        .unwrap_or(0);
    assert_eq!(backups, 0, "refused opens must not write backups");
}
