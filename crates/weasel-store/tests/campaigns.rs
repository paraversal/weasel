//! Campaign repository, service, stats and audience against a real database.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use weasel_core::{
    CoreError,
    campaign::{
        AudienceFilter, CampaignPatch, CampaignQuery, CampaignService, CampaignStats, NewCampaign,
        can_transition,
    },
    contact::{Contact, ContactPatch, ContactService, NewContact},
    enums::{CampaignStatus, ContactStatus},
    identity::NameIdentity,
    query::PageRequest,
    send::{SendService, format_recipients},
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
    clipboard: Arc<FakeClipboard>,
    clock: Arc<FixedClock>,
}

async fn harness() -> Harness {
    let db = TempDb::new().await;
    let store = db.store();
    let clock = Arc::new(FixedClock::new(1_000));
    let contacts_repo = SqliteContacts::new(store.clone());
    let sends_repo = SqliteSends::new(store.clone());
    let clipboard = Arc::new(FakeClipboard::new());
    Harness {
        db,
        campaigns: CampaignService::new(
            Arc::new(SqliteCampaigns::new(store.clone())),
            clock.clone(),
        ),
        contacts: ContactService::new(
            Arc::new(contacts_repo),
            clock.clone(),
            Arc::new(NameIdentity),
        ),
        sends: SendService::new(Arc::new(sends_repo), clipboard.clone(), clock.clone()),
        clipboard,
        clock,
    }
}

fn campaign_input(name: &str) -> NewCampaign {
    NewCampaign {
        name: name.to_owned(),
        description: String::new(),
        subject: String::new(),
        body: String::new(),
    }
}

async fn create_campaign(harness: &Harness, name: &str) -> weasel_core::campaign::Campaign {
    harness
        .campaigns
        .create(campaign_input(name))
        .await
        .unwrap()
}

async fn activate(harness: &Harness, name: &str) -> weasel_core::campaign::Campaign {
    let campaign = create_campaign(harness, name).await;
    harness
        .campaigns
        .set_status(campaign.id, campaign.version, CampaignStatus::Active)
        .await
        .unwrap()
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

async fn set_status(harness: &Harness, contact: Contact, status: ContactStatus) -> Contact {
    harness
        .contacts
        .set_status(contact.id, contact.version, status)
        .await
        .unwrap()
}

#[tokio::test]
async fn transitions_table_and_rejected_moves() {
    use CampaignStatus::{Active, Archived, Draft};
    assert!(can_transition(Draft, Active));
    assert!(can_transition(Draft, Archived));
    assert!(can_transition(Active, Archived));
    assert!(can_transition(Archived, Active));
    assert!(!can_transition(Active, Draft));
    assert!(!can_transition(Archived, Draft));
    assert!(!can_transition(Draft, Draft));
    assert!(!can_transition(Active, Active));
    assert!(!can_transition(Archived, Archived));

    let harness = harness().await;
    let draft = create_campaign(&harness, "Draft").await;
    assert_eq!(draft.status, Draft);

    let err = harness
        .campaigns
        .set_status(draft.id, draft.version, Draft)
        .await
        .unwrap_err();
    assert!(matches!(err, CoreError::InvalidState(_)));

    let active = activate(&harness, "Active").await;
    let archived = harness
        .campaigns
        .set_status(active.id, active.version, Archived)
        .await
        .unwrap();
    assert_eq!(archived.status, Archived);
    let reactivated = harness
        .campaigns
        .set_status(archived.id, archived.version, Active)
        .await
        .unwrap();
    assert_eq!(reactivated.status, Active);

    let err = harness
        .campaigns
        .set_status(reactivated.id, 1, Archived)
        .await
        .unwrap_err();
    assert!(matches!(err, CoreError::Conflict(_)));
    assert_fk_clean(&harness.db.store()).await;
}

#[tokio::test]
async fn create_update_and_no_op() {
    let harness = harness().await;
    let campaign = create_campaign(&harness, "  Freshies  ").await;
    assert_eq!(campaign.name, "Freshies");
    assert_eq!(campaign.version, 1);
    assert_eq!(campaign.status, CampaignStatus::Draft);

    harness.clock.set(2_000);
    let unchanged = harness
        .campaigns
        .update(
            campaign.id,
            CampaignPatch {
                version: campaign.version,
                name: Some(" Freshies ".to_owned()),
                ..CampaignPatch::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(unchanged, campaign);

    let updated = harness
        .campaigns
        .update(
            campaign.id,
            CampaignPatch {
                version: campaign.version,
                description: Some("Spring roster".to_owned()),
                subject: Some("Hello".to_owned()),
                body: Some("Body".to_owned()),
                ..CampaignPatch::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(updated.version, 2);
    assert_eq!(updated.updated_at, 2_000);
    assert_eq!(updated.description, "Spring roster");
    assert_eq!(updated.status, CampaignStatus::Draft);

    let stale = harness
        .campaigns
        .update(
            campaign.id,
            CampaignPatch {
                version: 1,
                body: Some("Again".to_owned()),
                ..CampaignPatch::default()
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(stale, CoreError::Conflict(_)));

    let err = harness
        .campaigns
        .create(NewCampaign {
            name: "   ".to_owned(),
            description: String::new(),
            subject: String::new(),
            body: String::new(),
        })
        .await
        .unwrap_err();
    assert!(matches!(err, CoreError::Validation { field: "name", .. }));
    assert_fk_clean(&harness.db.store()).await;
}

#[tokio::test]
async fn delete_is_refused_with_batches() {
    let harness = harness().await;
    let empty = create_campaign(&harness, "Empty").await;
    harness
        .campaigns
        .delete(empty.id, empty.version)
        .await
        .unwrap();

    create_contact(&harness, "Ada", "Lovelace", "ada@example.com").await;
    let used = activate(&harness, "Used").await;
    harness.sends.prepare_send(used.id).await.unwrap();

    let err = harness
        .campaigns
        .delete(used.id, used.version)
        .await
        .unwrap_err();
    assert!(
        matches!(err, CoreError::InvalidState(message) if message.contains("archive")),
        "send history blocks campaign deletion"
    );
}

#[tokio::test]
async fn stats_and_audience_on_a_mixed_fixture() {
    let harness = harness().await;
    let _alice = create_contact(&harness, "Alice", "Able", "alice@example.com").await;
    let bob = create_contact(&harness, "Bob", "Baker", "bob@example.com").await;
    let cara = create_contact(&harness, "Cara", "Clark", "cara@example.com").await;
    let dan = create_contact(&harness, "Dan", "Davis", "dan@example.com").await;
    let eve = create_contact(&harness, "Eve", "Evans", "eve@example.com").await;
    let fay = create_contact(&harness, "Fay", "Frost", "fay@example.com").await;

    set_status(&harness, cara, ContactStatus::Removed).await;
    set_status(&harness, dan, ContactStatus::Archived).await;
    set_status(&harness, fay, ContactStatus::Archived).await;

    let campaign = activate(&harness, "Spring").await;
    // Audience at copy time: Alice, Bob, Eve (Cara removed, Dan/Fay archived).
    let prepared = harness.sends.prepare_send(campaign.id).await.unwrap();
    assert_eq!(prepared.emails.len(), 3);
    assert_eq!(
        harness.clipboard.writes().last().unwrap(),
        &format_recipients(&prepared.emails)
    );

    // Snapshot stays put when Bob changes address.
    let bob = harness
        .contacts
        .update(
            bob.id,
            ContactPatch {
                version: bob.version,
                email: Some("bob.baker@example.edu".to_owned()),
                ..ContactPatch::default()
            },
        )
        .await
        .unwrap();

    // A contact added afterwards is unsent.
    let henry = create_contact(&harness, "Henry", "Hughes", "henry@example.com").await;
    // Deleting a person with history is impossible through the service; a raw delete
    // simulates the FK path that nulls `contact_id`.
    sqlx::query("DELETE FROM contacts WHERE id = ?")
        .bind(eve.id)
        .execute(harness.db.store().pool())
        .await
        .unwrap();

    let stats: CampaignStats = harness.campaigns.stats(campaign.id).await.unwrap();
    assert_eq!(stats.sent, 3, "including the deleted person's record");
    assert_eq!(stats.unsent, 1);
    assert_eq!(stats.removed, 1);
    assert_eq!(stats.total, 4, "sent + unsent");
    assert_eq!(stats.last_batch_at, Some(1_000));

    let sent = harness
        .campaigns
        .audience(
            campaign.id,
            AudienceFilter::Sent,
            None,
            &PageRequest::new(100, 0),
        )
        .await
        .unwrap();
    assert_eq!(sent.total, 3);
    let bob_row = sent
        .items
        .iter()
        .find(|row| row.contact_id == Some(bob.id))
        .unwrap();
    assert_eq!(bob_row.email, "bob@example.com", "snapshot, not current");
    let eve_row = sent
        .items
        .iter()
        .find(|row| row.last_name == "Evans")
        .unwrap();
    assert_eq!(eve_row.contact_id, None);
    assert_eq!(eve_row.email, "eve@example.com");
    assert!(eve_row.contact_status.is_none());

    let unsent = harness
        .campaigns
        .audience(
            campaign.id,
            AudienceFilter::Unsent,
            None,
            &PageRequest::new(100, 0),
        )
        .await
        .unwrap();
    assert_eq!(unsent.total, 1);
    assert_eq!(unsent.items[0].contact_id, Some(henry.id));
    assert_eq!(unsent.items[0].contact_status, Some(ContactStatus::Active));

    let all = harness
        .campaigns
        .audience(
            campaign.id,
            AudienceFilter::All,
            None,
            &PageRequest::new(100, 0),
        )
        .await
        .unwrap();
    assert_eq!(all.total, 4);
    let names: Vec<&str> = all.items.iter().map(|row| row.last_name.as_str()).collect();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    assert_eq!(names, sorted, "all is ordered by last name");

    let searched = harness
        .campaigns
        .audience(
            campaign.id,
            AudienceFilter::All,
            Some("Baker"),
            &PageRequest::new(100, 0),
        )
        .await
        .unwrap();
    assert_eq!(searched.total, 1);
    assert_eq!(searched.items[0].last_name, "Baker");

    let paged = harness
        .campaigns
        .audience(
            campaign.id,
            AudienceFilter::All,
            None,
            &PageRequest::new(2, 2),
        )
        .await
        .unwrap();
    assert_eq!(paged.items.len(), 2);
    assert_eq!(paged.total, 4);

    let listed = harness
        .campaigns
        .list(&CampaignQuery {
            status: Some(CampaignStatus::Active),
            search: Some("Spring".to_owned()),
            page: PageRequest::new(10, 0),
        })
        .await
        .unwrap();
    assert_eq!(listed.total, 1);
    assert_eq!(listed.items[0].id, campaign.id);
    assert_fk_clean(&harness.db.store()).await;
    assert_integrity(&harness.db.store()).await;
}
