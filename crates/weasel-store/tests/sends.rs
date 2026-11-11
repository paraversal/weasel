//! Send batches: clipboard ordering, rollback, snapshots, recopy and concurrency.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use weasel_core::{
    CoreError,
    campaign::{CampaignPatch, CampaignService, NewCampaign},
    contact::{Contact, ContactPatch, ContactService, NewContact},
    enums::{CampaignStatus, ContactStatus},
    identity::NameIdentity,
    ports::SendRepository,
    query::PageRequest,
    send::{SendBatchQuery, SendService, format_recipients},
};
use weasel_store::{SqliteCampaigns, SqliteContacts, SqliteSends};
use weasel_testkit::{
    FakeClipboard, FixedClock, TempDb,
    assert::{assert_fk_clean, assert_integrity},
};

struct Harness {
    db: TempDb,
    campaigns: CampaignService,
    contacts: ContactService,
    sends: SendService,
    sends_repo: SqliteSends,
    clipboard: Arc<FakeClipboard>,
    clock: Arc<FixedClock>,
}

async fn harness() -> Harness {
    let db = TempDb::new().await;
    let store = db.store();
    let clock = Arc::new(FixedClock::new(1_000));
    let clipboard = Arc::new(FakeClipboard::new());
    let sends_repo = SqliteSends::new(store.clone());
    Harness {
        db,
        campaigns: CampaignService::new(
            Arc::new(SqliteCampaigns::new(store.clone())),
            clock.clone(),
        ),
        contacts: ContactService::new(
            Arc::new(SqliteContacts::new(store.clone())),
            clock.clone(),
            Arc::new(NameIdentity),
        ),
        sends: SendService::new(
            Arc::new(sends_repo.clone()),
            clipboard.clone(),
            clock.clone(),
        ),
        sends_repo,
        clipboard,
        clock,
    }
}

async fn create_contact(harness: &Harness, first: &str, last: &str, email: &str) -> Contact {
    harness
        .contacts
        .create_manual(NewContact {
            first_name: first.to_owned(),
            last_name: last.to_owned(),
            email: email.to_owned(),
            notes: String::new(),
        })
        .await
        .unwrap()
}

async fn active_campaign(harness: &Harness, name: &str) -> i64 {
    let campaign = harness
        .campaigns
        .create(NewCampaign {
            name: name.to_owned(),
            description: String::new(),
            subject: String::new(),
            body: String::new(),
        })
        .await
        .unwrap();
    harness
        .campaigns
        .set_status(campaign.id, campaign.version, CampaignStatus::Active)
        .await
        .unwrap()
        .id
}

async fn record_count(harness: &Harness, batch_id: i64) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM send_records WHERE batch_id = ?")
        .bind(batch_id)
        .fetch_one(harness.db.store().pool())
        .await
        .unwrap()
}

#[tokio::test]
async fn prepare_send_copies_before_commit_and_is_not_repeatable() {
    let harness = harness().await;
    create_contact(&harness, "Ada", "Lovelace", "ada@example.com").await;
    create_contact(&harness, "Bob", "Baker", "bob@example.com").await;
    create_contact(&harness, "Cid", "Clark", "cid@example.com").await;
    let removed = create_contact(&harness, "Rex", "Removed", "rex@example.com").await;
    harness
        .contacts
        .set_status(removed.id, removed.version, ContactStatus::Removed)
        .await
        .unwrap();
    let archived = create_contact(&harness, "Amy", "Archived", "amy@example.com").await;
    harness
        .contacts
        .set_status(archived.id, archived.version, ContactStatus::Archived)
        .await
        .unwrap();

    let campaign_id = active_campaign(&harness, "Spring").await;
    harness.clock.set(2_000);
    let prepared = harness.sends.prepare_send(campaign_id).await.unwrap();

    assert_eq!(prepared.emails.len(), 3);
    assert_eq!(prepared.batch.recipient_count, 3);
    assert_eq!(prepared.batch.copied_at, Some(2_000));
    assert_eq!(prepared.batch.created_at, 2_000);
    assert_eq!(
        harness.clipboard.writes(),
        vec![format_recipients(&prepared.emails)],
        "the clipboard is written exactly once, with the returned order"
    );

    let records = harness
        .sends
        .recipients(prepared.batch.id, None, &PageRequest::new(100, 0))
        .await
        .unwrap();
    assert_eq!(records.total, 3);
    let record_emails: Vec<&str> = records.items.iter().map(|row| row.email.as_str()).collect();
    let prepared_emails: Vec<&str> = prepared.emails.iter().map(String::as_str).collect();
    assert_eq!(record_emails, prepared_emails);

    let second = harness.sends.prepare_send(campaign_id).await.unwrap_err();
    assert!(matches!(second, CoreError::InvalidState(_)));
    assert_eq!(
        harness
            .sends
            .list(&SendBatchQuery::default())
            .await
            .unwrap()
            .total,
        1
    );

    // Someone added after the copy is still unsent.
    create_contact(&harness, "Dee", "Doe", "dee@example.com").await;
    let stats = harness.campaigns.stats(campaign_id).await.unwrap();
    assert_eq!(stats.sent, 3);
    assert_eq!(stats.unsent, 1);

    let listed = harness
        .sends
        .list(&SendBatchQuery {
            search: Some("Spring".to_owned()),
            ..SendBatchQuery::default()
        })
        .await
        .unwrap();
    assert_eq!(listed.total, 1);
    assert_eq!(listed.items[0].id, prepared.batch.id);
    assert_eq!(listed.items[0].campaign_name, "Spring");
    assert_fk_clean(&harness.db.store()).await;
}

#[tokio::test]
async fn clipboard_failure_records_nothing() {
    let harness = harness().await;
    create_contact(&harness, "Ada", "Lovelace", "ada@example.com").await;
    let campaign_id = active_campaign(&harness, "Spring").await;

    harness.clipboard.fail_next();
    let err = harness.sends.prepare_send(campaign_id).await.unwrap_err();
    assert!(matches!(err, CoreError::Io(_)), "got {err:?}");

    assert_eq!(
        harness
            .sends
            .list(&SendBatchQuery::default())
            .await
            .unwrap()
            .total,
        0
    );
    let records: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM send_records")
        .fetch_one(harness.db.store().pool())
        .await
        .unwrap();
    assert_eq!(records, 0);
    assert!(harness.clipboard.writes().is_empty());

    // The campaign is still sendable afterwards.
    let prepared = harness.sends.prepare_send(campaign_id).await.unwrap();
    assert_eq!(prepared.emails, vec!["ada@example.com".to_owned()]);
}

#[tokio::test]
async fn only_active_campaigns_can_be_sent() {
    let harness = harness().await;
    create_contact(&harness, "Ada", "Lovelace", "ada@example.com").await;

    let draft = harness
        .campaigns
        .create(NewCampaign {
            name: "Draft".to_owned(),
            description: String::new(),
            subject: String::new(),
            body: String::new(),
        })
        .await
        .unwrap();
    let err = harness.sends.prepare_send(draft.id).await.unwrap_err();
    assert!(
        matches!(err, CoreError::InvalidState(message) if message.contains("active")),
        "draft campaign must be refused"
    );

    let campaign_id = active_campaign(&harness, "Archived").await;
    let active = harness.campaigns.get(campaign_id).await.unwrap();
    harness
        .campaigns
        .set_status(active.id, active.version, CampaignStatus::Archived)
        .await
        .unwrap();
    let err = harness.sends.prepare_send(campaign_id).await.unwrap_err();
    assert!(matches!(err, CoreError::InvalidState(message) if message.contains("active")));

    let missing = harness.sends.prepare_send(999).await.unwrap_err();
    assert!(matches!(missing, CoreError::NotFound { .. }));
}

#[tokio::test]
async fn recopy_returns_the_original_snapshot() {
    let harness = harness().await;
    let bob = create_contact(&harness, "Bob", "Baker", "bob@example.com").await;
    create_contact(&harness, "Ada", "Lovelace", "ada@example.com").await;
    let campaign_id = active_campaign(&harness, "Spring").await;

    let prepared = harness.sends.prepare_send(campaign_id).await.unwrap();
    harness
        .contacts
        .update(
            bob.id,
            ContactPatch {
                version: bob.version,
                email: Some("bob@example.edu".to_owned()),
                ..ContactPatch::default()
            },
        )
        .await
        .unwrap();

    harness.clock.set(3_000);
    let recopied = harness.sends.recopy(prepared.batch.id).await.unwrap();
    assert_eq!(recopied.emails, prepared.emails);
    assert!(recopied.emails.contains(&"bob@example.com".to_owned()));
    assert_eq!(recopied.batch.copied_at, Some(3_000));
    assert_eq!(harness.clipboard.writes().len(), 2);
    assert_eq!(
        harness.clipboard.writes()[1],
        format_recipients(&prepared.emails)
    );

    let blocked = sqlx::query("UPDATE send_records SET email = 'rewritten@example.com'")
        .execute(harness.db.store().pool())
        .await;
    assert!(blocked.is_err(), "the snapshot trigger blocks updates");
    assert_integrity(&harness.db.store()).await;
}

#[tokio::test]
async fn deleted_contact_keeps_its_snapshot_in_the_audience() {
    let harness = harness().await;
    let ada = create_contact(&harness, "Ada", "Lovelace", "ada@example.com").await;
    create_contact(&harness, "Bob", "Baker", "bob@example.com").await;
    let campaign_id = active_campaign(&harness, "Spring").await;
    harness.sends.prepare_send(campaign_id).await.unwrap();

    sqlx::query("DELETE FROM contacts WHERE id = ?")
        .bind(ada.id)
        .execute(harness.db.store().pool())
        .await
        .unwrap();

    let sent = harness
        .campaigns
        .audience(
            campaign_id,
            weasel_core::campaign::AudienceFilter::Sent,
            None,
            &PageRequest::new(100, 0),
        )
        .await
        .unwrap();
    assert_eq!(sent.total, 2);
    let deleted = sent
        .items
        .iter()
        .find(|row| row.last_name == "Lovelace")
        .unwrap();
    assert_eq!(deleted.contact_id, None);
    assert_eq!(deleted.email, "ada@example.com");
    assert!(deleted.sent_at.is_some());
    assert_fk_clean(&harness.db.store()).await;
}

#[tokio::test]
async fn note_and_search() {
    let harness = harness().await;
    create_contact(&harness, "Ada", "Lovelace", "ada@example.com").await;
    let campaign_id = active_campaign(&harness, "Spring").await;
    let prepared = harness.sends.prepare_send(campaign_id).await.unwrap();

    let noted = harness
        .sends
        .set_note(prepared.batch.id, "St. Patrick's day".to_owned())
        .await
        .unwrap();
    assert_eq!(noted.note, "St. Patrick's day");

    let by_note = harness.sends.get(prepared.batch.id).await.unwrap();
    assert_eq!(by_note.note, "St. Patrick's day");

    let searched = harness
        .sends_repo
        .list(&SendBatchQuery {
            search: Some("Lovelace".to_owned()),
            ..SendBatchQuery::default()
        })
        .await
        .unwrap();
    assert_eq!(searched.total, 1);

    let no_match = harness
        .sends_repo
        .list(&SendBatchQuery {
            search: Some("Nobody".to_owned()),
            ..SendBatchQuery::default()
        })
        .await
        .unwrap();
    assert_eq!(no_match.total, 0);

    let recipients = harness
        .sends
        .recipients(
            prepared.batch.id,
            Some("ada@example"),
            &PageRequest::new(10, 0),
        )
        .await
        .unwrap();
    assert_eq!(recipients.total, 1);
    let recipients = harness
        .sends
        .recipients(prepared.batch.id, Some("nobody"), &PageRequest::new(10, 0))
        .await
        .unwrap();
    assert_eq!(recipients.total, 0);

    let too_long = "x".repeat(weasel_core::limits::BATCH_NOTE_MAX + 1);
    let err = harness
        .sends
        .set_note(prepared.batch.id, too_long)
        .await
        .unwrap_err();
    assert!(matches!(err, CoreError::Validation { field: "note", .. }));

    let missing = harness.sends.get(999).await.unwrap_err();
    assert!(matches!(missing, CoreError::NotFound { .. }));
}

#[tokio::test]
async fn two_prepares_race_and_exactly_one_wins() {
    let harness = harness().await;
    create_contact(&harness, "Ada", "Lovelace", "ada@example.com").await;
    create_contact(&harness, "Bob", "Baker", "bob@example.com").await;
    let campaign_id = active_campaign(&harness, "Spring").await;

    let service = &harness.sends;
    let (left, right) = tokio::join!(
        service.prepare_send(campaign_id),
        service.prepare_send(campaign_id)
    );

    let successes = [&left, &right]
        .iter()
        .filter(|result| result.is_ok())
        .count();
    let failures = [&left, &right]
        .iter()
        .filter(|result| matches!(result, Err(CoreError::InvalidState(_))))
        .count();
    assert_eq!(successes, 1, "begin immediate serializes the two batches");
    assert_eq!(failures, 1);

    let batch_id = match (left, right) {
        (Ok(prepared), Err(_)) | (Err(_), Ok(prepared)) => prepared.batch.id,
        (Ok(_), Ok(_)) => panic!("both prepares succeeded"),
        (Err(err), Err(_)) => panic!("no prepare succeeded: {err:?}"),
    };
    assert_eq!(record_count(&harness, batch_id).await, 2);
    let batches: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM send_batches")
        .fetch_one(harness.db.store().pool())
        .await
        .unwrap();
    assert_eq!(batches, 1);
    assert_eq!(harness.clipboard.writes().len(), 1);
    assert_fk_clean(&harness.db.store()).await;
}

#[tokio::test]
async fn campaign_edit_is_audited() {
    let harness = harness().await;
    create_contact(&harness, "Ada", "Lovelace", "ada@example.com").await;
    let campaign_id = active_campaign(&harness, "Spring").await;
    let prepared = harness.sends.prepare_send(campaign_id).await.unwrap();

    let events = harness.sends_repo.get(prepared.batch.id).await.unwrap();
    assert_eq!(events.note, "");

    // Audit rows exist for the batch creation and the campaign transition.
    let rows: Vec<String> = sqlx::query_scalar(
        "SELECT action FROM audit_events WHERE entity_type IN ('campaign', 'send_batch') \
         ORDER BY id",
    )
    .fetch_all(harness.db.store().pool())
    .await
    .unwrap();
    assert!(rows.contains(&"campaign.create".to_owned()));
    assert!(rows.contains(&"campaign.status".to_owned()));
    assert!(rows.contains(&"send_batch.create".to_owned()));

    let created = harness.campaigns.get(campaign_id).await.unwrap();
    harness
        .campaigns
        .update(
            campaign_id,
            CampaignPatch {
                version: created.version,
                name: Some("Autumn".to_owned()),
                ..CampaignPatch::default()
            },
        )
        .await
        .unwrap();
    let rows: Vec<String> = sqlx::query_scalar(
        "SELECT action FROM audit_events WHERE entity_type = 'campaign' ORDER BY id",
    )
    .fetch_all(harness.db.store().pool())
    .await
    .unwrap();
    assert!(rows.contains(&"campaign.update".to_owned()));
}
