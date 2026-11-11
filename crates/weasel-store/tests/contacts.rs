//! Contact repository and service against a real migrated database.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use weasel_core::{
    CoreError,
    audit::actions,
    contact::{Contact, ContactPatch, ContactQuery, ContactService, ContactSort, NewContact},
    enums::{ContactSource, ContactStatus},
    identity::NameIdentity,
    ports::AuditRepository,
    query::PageRequest,
};
use weasel_store::{SqliteAudit, SqliteContacts, get_meta};
use weasel_testkit::{
    FixedClock, TempDb,
    assert::{assert_fk_clean, assert_integrity},
};

struct Harness {
    db: TempDb,
    service: ContactService,
    audit: SqliteAudit,
    clock: Arc<FixedClock>,
}

async fn harness() -> Harness {
    let db = TempDb::new().await;
    let store = db.store();
    let contacts = SqliteContacts::new(store.clone());
    let audit = SqliteAudit::new(store);
    let clock = Arc::new(FixedClock::new(1_000));
    let service = ContactService::new(
        Arc::new(contacts.clone()),
        clock.clone(),
        Arc::new(NameIdentity),
    );
    Harness {
        db,
        service,
        audit,
        clock,
    }
}

fn input(first: &str, last: &str, email: &str) -> NewContact {
    NewContact {
        first_name: first.to_owned(),
        last_name: last.to_owned(),
        email: email.to_owned(),
        notes: String::new(),
    }
}

async fn create(harness: &Harness, first: &str, last: &str, email: &str) -> Contact {
    harness
        .service
        .create_manual(input(first, last, email))
        .await
        .unwrap()
}

async fn audit_actions(harness: &Harness, id: i64) -> Vec<String> {
    harness
        .audit
        .for_entity("contact", &id.to_string(), &PageRequest::new(100, 0))
        .await
        .unwrap()
        .items
        .into_iter()
        .map(|event| event.action)
        .collect()
}

#[tokio::test]
async fn create_and_get_round_trip() {
    let harness = harness().await;
    let contact = create(&harness, "  Ada ", "Lovelace", " Ada@Example.COM ").await;

    assert_eq!(contact.first_name, "Ada");
    assert_eq!(contact.last_name, "Lovelace");
    assert_eq!(contact.email, "Ada@Example.COM");
    assert_eq!(contact.email_normalized, "ada@example.com");
    assert_eq!(contact.identity_key.as_deref(), Some("ada\u{1f}lovelace"));
    assert_eq!(contact.status, ContactStatus::Active);
    assert_eq!(contact.source, ContactSource::Manual);
    assert_eq!(contact.version, 1);
    assert_eq!(contact.date_added, 1_000);
    assert_eq!(contact.updated_at, 1_000);

    let stored = harness.service.get(contact.id).await.unwrap();
    assert_eq!(stored, contact);

    let detail = harness.service.detail(contact.id).await.unwrap();
    assert_eq!(detail.email_history.len(), 1);
    assert_eq!(detail.email_history[0].email, "Ada@Example.COM");
    assert!(detail.email_history[0].valid_to.is_none());
    assert!(detail.campaigns.is_empty());

    assert_eq!(
        get_meta(&harness.db.store(), "identity_strategy")
            .await
            .unwrap()
            .as_deref(),
        Some("name-v1")
    );

    let actions = audit_actions(&harness, contact.id).await;
    assert_eq!(actions, vec![actions::CONTACT_CREATE.to_owned()]);

    assert_fk_clean(&harness.db.store()).await;
    assert_integrity(&harness.db.store()).await;
}

#[tokio::test]
async fn duplicate_live_email_is_a_conflict() {
    let harness = harness().await;
    create(&harness, "Ada", "Lovelace", "ada@example.com").await;

    let err = harness
        .service
        .create_manual(input("Grace", "Hopper", "ADA@example.com"))
        .await
        .unwrap_err();
    assert!(matches!(err, CoreError::Conflict(message) if message.contains("email")));

    assert_eq!(harness.service.count_active().await.unwrap(), 1);
    assert_fk_clean(&harness.db.store()).await;
}

#[tokio::test]
async fn archiving_frees_the_email_but_unarchiving_into_a_taken_one_conflicts() {
    let harness = harness().await;
    let ada = create(&harness, "Ada", "Lovelace", "ada@example.com").await;
    let archived = harness
        .service
        .set_status(ada.id, ada.version, ContactStatus::Archived)
        .await
        .unwrap();
    assert_eq!(archived.version, 2);

    let grace = create(&harness, "Grace", "Hopper", "ada@example.com").await;

    let err = harness
        .service
        .set_status(archived.id, archived.version, ContactStatus::Active)
        .await
        .unwrap_err();
    assert!(matches!(err, CoreError::Conflict(message) if message.contains("email")));

    // Grace still owns the address; Ada stays archived.
    let stored = harness.service.get(archived.id).await.unwrap();
    assert_eq!(stored.status, ContactStatus::Archived);
    assert_eq!(
        harness.service.get(grace.id).await.unwrap().status,
        ContactStatus::Active
    );
    assert_fk_clean(&harness.db.store()).await;
}

#[tokio::test]
async fn stale_versions_and_missing_rows_are_distinguished() {
    let harness = harness().await;
    let ada = create(&harness, "Ada", "Lovelace", "ada@example.com").await;

    let stale = ContactPatch {
        version: 0,
        first_name: Some("Augusta".to_owned()),
        ..ContactPatch::default()
    };
    let err = harness.service.update(ada.id, stale).await.unwrap_err();
    assert!(matches!(err, CoreError::Conflict(_)));

    let err = harness.service.get(999).await.unwrap_err();
    assert!(matches!(
        err,
        CoreError::NotFound {
            entity: "contact",
            ..
        }
    ));
    let err = harness.service.delete(999, 1).await.unwrap_err();
    assert!(matches!(err, CoreError::NotFound { .. }));
}

#[tokio::test]
async fn no_op_patch_does_not_write() {
    let harness = harness().await;
    let ada = create(&harness, "Ada", "Lovelace", "ada@example.com").await;

    let patch = ContactPatch {
        version: ada.version,
        first_name: Some("  Ada ".to_owned()),
        notes: Some(String::new()),
        ..ContactPatch::default()
    };
    let unchanged = harness.service.update(ada.id, patch).await.unwrap();
    assert_eq!(unchanged, ada);

    // A stale no-op still returns the current row: nothing was written.
    let patch = ContactPatch {
        version: 0,
        first_name: Some("Ada".to_owned()),
        ..ContactPatch::default()
    };
    let unchanged = harness.service.update(ada.id, patch).await.unwrap();
    assert_eq!(unchanged, ada);
    assert_eq!(audit_actions(&harness, ada.id).await.len(), 1);
}

#[tokio::test]
async fn email_change_closes_history_and_is_audited() {
    let harness = harness().await;
    let ada = create(&harness, "Ada", "Lovelace", "ada@example.com").await;
    harness.clock.set(2_000);

    let patch = ContactPatch {
        version: ada.version,
        email: Some("ada.lovelace@example.edu".to_owned()),
        ..ContactPatch::default()
    };
    let updated = harness.service.update(ada.id, patch).await.unwrap();
    assert_eq!(updated.email_normalized, "ada.lovelace@example.edu");
    assert_eq!(updated.version, 2);
    assert_eq!(updated.updated_at, 2_000);

    let detail = harness.service.detail(ada.id).await.unwrap();
    assert_eq!(detail.email_history.len(), 2);
    assert_eq!(detail.email_history[0].email, "ada.lovelace@example.edu");
    assert!(detail.email_history[0].valid_to.is_none());
    assert_eq!(detail.email_history[1].valid_to, Some(2_000));

    let events = harness
        .audit
        .for_entity("contact", &ada.id.to_string(), &PageRequest::new(100, 0))
        .await
        .unwrap()
        .items;
    assert_eq!(events[0].action, actions::CONTACT_EMAIL_CHANGE);
    assert_eq!(events[0].metadata["reason"], "manual");
    assert_eq!(
        events[0].before.as_ref().unwrap()["email"],
        "ada@example.com"
    );
    assert_eq!(
        events[0].after.as_ref().unwrap()["email"],
        "ada.lovelace@example.edu"
    );
    assert_fk_clean(&harness.db.store()).await;
}

#[tokio::test]
async fn delete_is_refused_with_send_history_and_keeps_the_audit() {
    let harness = harness().await;
    let ada = create(&harness, "Ada", "Lovelace", "ada@example.com").await;
    let store = harness.db.store();
    let pool = store.pool();

    let campaign_id: i64 = sqlx::query_scalar(
        "INSERT INTO campaigns (name, status, created_at, updated_at) \
         VALUES ('Freshies', 'active', 1, 1) RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let batch_id: i64 = sqlx::query_scalar(
        "INSERT INTO send_batches (campaign_id, created_at, recipient_count) \
         VALUES (?, 1, 1) RETURNING id",
    )
    .bind(campaign_id)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO send_records \
         (batch_id, contact_id, email, email_normalized, first_name, last_name, created_at) \
         VALUES (?, ?, 'ada@example.com', 'ada@example.com', 'Ada', 'Lovelace', 1)",
    )
    .bind(batch_id)
    .bind(ada.id)
    .execute(pool)
    .await
    .unwrap();

    let err = harness
        .service
        .delete(ada.id, ada.version)
        .await
        .unwrap_err();
    assert!(
        matches!(err, CoreError::InvalidState(message) if message.contains("archive")),
        "send history must block hard deletes"
    );

    let grace = create(&harness, "Grace", "Hopper", "grace@example.com").await;
    harness
        .service
        .delete(grace.id, grace.version)
        .await
        .unwrap();

    let history: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM contact_email_history WHERE contact_id = ?")
            .bind(grace.id)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(history, 0);
    assert!(
        audit_actions(&harness, grace.id)
            .await
            .contains(&actions::CONTACT_DELETE.to_owned())
    );

    let receipt = harness.service.detail(ada.id).await.unwrap();
    assert_eq!(receipt.campaigns.len(), 1);
    assert_eq!(receipt.campaigns[0].campaign_name, "Freshies");
    assert_fk_clean(&harness.db.store()).await;
}

#[tokio::test]
async fn list_filters_sorts_and_pages() {
    let harness = harness().await;
    create(&harness, "100%", "Percent", "pct@example.com").await;
    create(&harness, "a_b", "Under", "under@example.com").await;
    create(&harness, "Ada", "Same", "ada@example.com").await;
    create(&harness, "Bob", "Same", "bob@example.com").await;
    let archived = create(&harness, "Arch", "Ived", "arch@example.com").await;
    harness
        .service
        .set_status(archived.id, archived.version, ContactStatus::Archived)
        .await
        .unwrap();

    let all = harness
        .service
        .list(&ContactQuery::default())
        .await
        .unwrap();
    assert_eq!(all.total, 4, "archived people are opt-in");
    assert!(!all.items.iter().any(|c| c.id == archived.id));

    let percent = harness
        .service
        .list(&ContactQuery {
            search: Some("%".to_owned()),
            ..ContactQuery::default()
        })
        .await
        .unwrap();
    assert_eq!(percent.total, 1);
    assert_eq!(percent.items[0].first_name, "100%");

    let underscore = harness
        .service
        .list(&ContactQuery {
            search: Some("_".to_owned()),
            ..ContactQuery::default()
        })
        .await
        .unwrap();
    assert_eq!(underscore.total, 1);
    assert_eq!(underscore.items[0].first_name, "a_b");

    let by_name = harness
        .service
        .list(&ContactQuery {
            sort: ContactSort::LastNameAsc,
            ..ContactQuery::default()
        })
        .await
        .unwrap();
    let names: Vec<(&str, &str)> = by_name
        .items
        .iter()
        .map(|c| (c.last_name.as_str(), c.first_name.as_str()))
        .collect();
    assert_eq!(
        names,
        vec![
            ("Percent", "100%"),
            ("Same", "Ada"),
            ("Same", "Bob"),
            ("Under", "a_b"),
        ]
    );

    let by_added = harness
        .service
        .list(&ContactQuery {
            sort: ContactSort::AddedDesc,
            ..ContactQuery::default()
        })
        .await
        .unwrap();
    let ids: Vec<i64> = by_added.items.iter().map(|c| c.id).collect();
    let mut descending = ids.clone();
    descending.sort_unstable_by(|a, b| b.cmp(a));
    assert_eq!(ids, descending, "ties break by id, newest first");

    let by_email = harness
        .service
        .list(&ContactQuery {
            sort: ContactSort::EmailAsc,
            ..ContactQuery::default()
        })
        .await
        .unwrap();
    let emails: Vec<&str> = by_email
        .items
        .iter()
        .map(|c| c.email_normalized.as_str())
        .collect();
    assert_eq!(
        emails,
        vec![
            "ada@example.com",
            "bob@example.com",
            "pct@example.com",
            "under@example.com",
        ]
    );

    let paged = harness
        .service
        .list(&ContactQuery {
            page: PageRequest::new(2, 1),
            ..ContactQuery::default()
        })
        .await
        .unwrap();
    assert_eq!(paged.items.len(), 2);
    assert_eq!(paged.total, 4);
    assert_eq!(paged.limit, 2);
    assert_eq!(paged.offset, 1);

    let clamped = harness
        .service
        .list(&ContactQuery {
            page: PageRequest::new(1_000, 0),
            ..ContactQuery::default()
        })
        .await
        .unwrap();
    assert_eq!(clamped.limit, 500);

    let stats = harness.service.stats().await.unwrap();
    assert_eq!(stats.active, 4);
    assert_eq!(stats.archived, 1);
    assert_eq!(stats.removed, 0);
    assert_eq!(stats.total, 5);
    assert_eq!(harness.service.count_active().await.unwrap(), 4);
    assert_fk_clean(&harness.db.store()).await;
}

#[tokio::test]
async fn every_stored_enum_value_decodes_and_unknown_ones_are_rejected() {
    let harness = harness().await;
    let store = harness.db.store();
    let pool = store.pool();

    for (index, status) in [
        ContactStatus::Active,
        ContactStatus::Removed,
        ContactStatus::Archived,
    ]
    .into_iter()
    .enumerate()
    {
        let email = format!("status{index}@example.com");
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO contacts \
             (first_name, last_name, email, email_normalized, status, source, date_added, updated_at) \
             VALUES ('A', 'B', ?, ?, ?, 'manual', 1, 1) RETURNING id",
        )
        .bind(&email)
        .bind(&email)
        .bind(status.as_str())
        .fetch_one(pool)
        .await
        .unwrap();
        assert_eq!(harness.service.get(id).await.unwrap().status, status);
    }

    for (index, source) in [
        ContactSource::Import,
        ContactSource::Manual,
        ContactSource::Api,
    ]
    .into_iter()
    .enumerate()
    {
        let email = format!("source{index}@example.com");
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO contacts \
             (first_name, last_name, email, email_normalized, status, source, date_added, updated_at) \
             VALUES ('A', 'B', ?, ?, 'archived', ?, 1, 1) RETURNING id",
        )
        .bind(&email)
        .bind(&email)
        .bind(source.as_str())
        .fetch_one(pool)
        .await
        .unwrap();
        assert_eq!(harness.service.get(id).await.unwrap().source, source);
    }

    let rejected = sqlx::query(
        "INSERT INTO contacts \
         (first_name, last_name, email, email_normalized, status, source, date_added, updated_at) \
         VALUES ('A', 'B', 'bad@example.com', 'bad@example.com', 'legacy', 'manual', 1, 1)",
    )
    .execute(pool)
    .await;
    assert!(rejected.is_err(), "CHECK constraints reject legacy values");
}
