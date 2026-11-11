#![allow(clippy::unwrap_used, clippy::expect_used)]

use weasel_store::{OpenOptions, open, restore_backup};
use weasel_testkit::assert;

#[tokio::test]
async fn restoring_a_backup_replaces_the_database() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("weasel.sqlite3");
    let backups_dir = dir.path().join("backups");
    let opts = OpenOptions {
        db_path: &db_path,
        backups_dir: &backups_dir,
    };

    let (store, _) = open(opts).await.unwrap();
    sqlx::query(
        "INSERT INTO contacts (first_name, last_name, email, email_normalized, date_added, updated_at) \
         VALUES ('Ada', 'Lovelace', 'ada@example.edu', 'ada@example.edu', 1, 1)",
    )
    .execute(store.pool())
    .await
    .unwrap();
    let backup = store.backup_now(&backups_dir).await.unwrap();

    sqlx::query("DELETE FROM contacts")
        .execute(store.pool())
        .await
        .unwrap();
    store.close().await;

    restore_backup(&backup, &db_path).unwrap();

    let (store, report) = open(opts).await.unwrap();
    assert_eq!(report.schema_version, 3);
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM contacts")
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(count, 1);
    assert::assert_integrity(&store).await;
    assert::assert_fk_clean(&store).await;
}
