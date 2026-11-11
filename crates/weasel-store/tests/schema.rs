#![allow(clippy::unwrap_used, clippy::expect_used)]

use sqlx::Row;
use weasel_store::Store;
use weasel_testkit::TempDb;

/// Every object the domain schema creates. Renaming or dropping one is a migration,
/// not a refactor; this test exists to make that explicit in review.
#[tokio::test]
async fn schema_snapshot_is_stable() {
    let db = TempDb::new().await;
    let store = db.store();
    let rows = sqlx::query(
        "SELECT type, name FROM sqlite_master \
         WHERE type IN ('table', 'index', 'trigger') \
           AND name NOT LIKE 'sqlite_%' \
           AND name <> '_sqlx_migrations' \
         ORDER BY name",
    )
    .fetch_all(store.pool())
    .await
    .unwrap();

    let mut actual: Vec<String> = rows
        .iter()
        .map(|row| {
            format!(
                "{}:{}",
                row.get::<String, _>("type"),
                row.get::<String, _>("name")
            )
        })
        .collect();
    actual.sort();

    let mut expected = vec![
        "table:app_meta".to_owned(),
        "table:audit_events".to_owned(),
        "table:campaigns".to_owned(),
        "table:contact_email_history".to_owned(),
        "table:contacts".to_owned(),
        "table:import_batches".to_owned(),
        "table:send_batches".to_owned(),
        "table:send_records".to_owned(),
        "table:settings".to_owned(),
        "index:ix_audit_entity".to_owned(),
        "index:ix_audit_time".to_owned(),
        "index:ix_campaigns_status".to_owned(),
        "index:ix_contact_email_history_contact".to_owned(),
        "index:ix_contact_email_history_lookup".to_owned(),
        "index:ix_contacts_batch".to_owned(),
        "index:ix_contacts_identity".to_owned(),
        "index:ix_contacts_name".to_owned(),
        "index:ix_contacts_status".to_owned(),
        "index:ix_import_batches_created".to_owned(),
        "index:ix_import_batches_status".to_owned(),
        "index:ix_send_batches_campaign".to_owned(),
        "index:ix_send_records_batch".to_owned(),
        "index:ix_send_records_contact".to_owned(),
        "index:ix_send_records_email".to_owned(),
        "index:ux_contacts_email_live".to_owned(),
        "index:ux_email_history_open".to_owned(),
        "trigger:audit_events_no_delete".to_owned(),
        "trigger:audit_events_no_update".to_owned(),
        "trigger:send_records_snapshot_immutable".to_owned(),
    ];
    expected.sort();

    assert_eq!(actual, expected);
}

async fn insert_contact(
    store: &Store,
    email: &str,
    status: &str,
) -> Result<sqlx::sqlite::SqliteQueryResult, sqlx::Error> {
    sqlx::query(
        "INSERT INTO contacts \
         (first_name, last_name, email, email_normalized, status, source, date_added, updated_at) \
         VALUES ('Ada', 'Lovelace', ?, ?, ?, 'manual', 1, 1)",
    )
    .bind(email)
    .bind(email.to_lowercase())
    .bind(status)
    .execute(store.pool())
    .await
}

#[tokio::test]
async fn live_email_is_unique_but_archived_rows_do_not_block() {
    let db = TempDb::new().await;
    let store = db.store();

    insert_contact(&store, "ada@example.edu", "active")
        .await
        .unwrap();
    // A removed contact still occupies the address so it can be restored.
    insert_contact(&store, "ADA@example.edu", "removed")
        .await
        .unwrap_err();
    // Archived rows are out of the way.
    insert_contact(&store, "ada@example.edu", "archived")
        .await
        .unwrap();
    weasel_testkit::assert::assert_fk_clean(&store).await;
}

#[tokio::test]
async fn only_one_open_email_history_row_per_contact() {
    let db = TempDb::new().await;
    let store = db.store();
    let contact_id: i64 =
        sqlx::query_scalar("INSERT INTO contacts (first_name, last_name, email, email_normalized, \
                            date_added, updated_at) VALUES ('A', 'B', 'a@x.edu', 'a@x.edu', 1, 1) \
                            RETURNING id")
            .fetch_one(store.pool())
            .await
            .unwrap();

    let insert = |valid_to: Option<i64>| {
        sqlx::query(
            "INSERT INTO contact_email_history \
             (contact_id, email, email_normalized, valid_from, valid_to, reason) \
             VALUES (?, 'a@x.edu', 'a@x.edu', 1, ?, 'import')",
        )
        .bind(contact_id)
        .bind(valid_to)
        .execute(store.pool())
    };

    insert(None).await.unwrap();
    insert(None).await.unwrap_err();
    insert(Some(2)).await.unwrap();
}

#[tokio::test]
async fn send_record_snapshot_is_immutable() {
    let db = TempDb::new().await;
    let store = db.store();
    let campaign_id: i64 = sqlx::query_scalar(
        "INSERT INTO campaigns (name, created_at, updated_at) VALUES ('C', 1, 1) RETURNING id",
    )
    .fetch_one(store.pool())
    .await
    .unwrap();
    let batch_id: i64 = sqlx::query_scalar(
        "INSERT INTO send_batches (campaign_id, created_at, recipient_count) VALUES (?, 1, 1) \
         RETURNING id",
    )
    .bind(campaign_id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    let record_id: i64 = sqlx::query_scalar(
        "INSERT INTO send_records (batch_id, email, email_normalized, first_name, last_name, created_at) \
         VALUES (?, 'a@x.edu', 'a@x.edu', 'A', 'B', 1) RETURNING id",
    )
    .bind(batch_id)
    .fetch_one(store.pool())
    .await
    .unwrap();

    for sql in [
        "UPDATE send_records SET email = 'b@x.edu' WHERE id = ?",
        "UPDATE send_records SET email_normalized = 'b@x.edu' WHERE id = ?",
        "UPDATE send_records SET first_name = 'C' WHERE id = ?",
        "UPDATE send_records SET last_name = 'C' WHERE id = ?",
        "UPDATE send_records SET batch_id = batch_id + 1 WHERE id = ?",
    ] {
        let err = sqlx::query(sql)
            .bind(record_id)
            .execute(store.pool())
            .await
            .unwrap_err();
        assert!(
            err.to_string().contains("snapshot is immutable"),
            "unexpected error: {err}"
        );
    }

    sqlx::query("UPDATE send_records SET status = 'sent' WHERE id = ?")
        .bind(record_id)
        .execute(store.pool())
        .await
        .unwrap();
}

#[tokio::test]
async fn audit_events_cannot_be_updated_or_deleted() {
    let db = TempDb::new().await;
    let store = db.store();
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO audit_events (at, action, entity_type) VALUES (1, 'contact.create', 'contact') \
         RETURNING id",
    )
    .fetch_one(store.pool())
    .await
    .unwrap();

    for sql in [
        "UPDATE audit_events SET action = 'other' WHERE id = ?",
        "DELETE FROM audit_events WHERE id = ?",
    ] {
        let err = sqlx::query(sql)
            .bind(id)
            .execute(store.pool())
            .await
            .unwrap_err();
        assert!(
            err.to_string().contains("append-only"),
            "unexpected error: {err}"
        );
    }
}

#[tokio::test]
async fn enum_check_constraints_reject_unknown_values() {
    let db = TempDb::new().await;
    let store = db.store();
    let contact_id: i64 =
        sqlx::query_scalar("INSERT INTO contacts (first_name, last_name, email, email_normalized, \
                            date_added, updated_at) VALUES ('A', 'B', 'a@x.edu', 'a@x.edu', 1, 1) \
                            RETURNING id")
            .fetch_one(store.pool())
            .await
            .unwrap();
    let campaign_id: i64 = sqlx::query_scalar(
        "INSERT INTO campaigns (name, created_at, updated_at) VALUES ('C', 1, 1) RETURNING id",
    )
    .fetch_one(store.pool())
    .await
    .unwrap();
    let batch_id: i64 = sqlx::query_scalar(
        "INSERT INTO send_batches (campaign_id, created_at, recipient_count) VALUES (?, 1, 1) \
         RETURNING id",
    )
    .bind(campaign_id)
    .fetch_one(store.pool())
    .await
    .unwrap();

    let cases: [(&str, &str); 8] = [
        (
            "INSERT INTO import_batches (source_kind, source_name, created_at) VALUES ('pdf', 'x', 1)",
            "source_kind",
        ),
        (
            "INSERT INTO import_batches (source_kind, source_name, scope, created_at) \
             VALUES ('csv', 'x', 'all', 1)",
            "scope",
        ),
        (
            "INSERT INTO import_batches (source_kind, source_name, status, created_at) \
             VALUES ('csv', 'x', 'done', 1)",
            "status",
        ),
        (
            "INSERT INTO contacts (first_name, last_name, email, email_normalized, status, \
             date_added, updated_at) VALUES ('A', 'B', 'b@x.edu', 'b@x.edu', 'deleted', 1, 1)",
            "status",
        ),
        (
            "INSERT INTO contacts (first_name, last_name, email, email_normalized, source, \
             date_added, updated_at) VALUES ('A', 'B', 'c@x.edu', 'c@x.edu', 'legacy', 1, 1)",
            "source",
        ),
        (
            "INSERT INTO campaigns (name, status, created_at, updated_at) \
             VALUES ('C2', 'paused', 1, 1)",
            "status",
        ),
        (
            "INSERT INTO send_records (batch_id, email, email_normalized, first_name, last_name, \
             status, created_at) VALUES (?, 'b@x.edu', 'b@x.edu', 'A', 'B', 'bounced', 1)",
            "status",
        ),
        (
            "INSERT INTO contact_email_history (contact_id, email, email_normalized, valid_from, \
             reason) VALUES (?, 'a@x.edu', 'a@x.edu', 1, 'other')",
            "reason",
        ),
    ];

    for (sql, column) in cases {
        let mut query = sqlx::query(sql);
        if sql.contains("send_records") {
            query = query.bind(batch_id);
        }
        if sql.contains("contact_email_history") {
            query = query.bind(contact_id);
        }
        let err = query.execute(store.pool()).await.unwrap_err();
        assert!(
            err.to_string().contains(column),
            "expected {column} CHECK failure, got: {err}"
        );
    }
}
