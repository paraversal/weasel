//! End-to-end import batches against a real database.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;
use std::sync::Arc;

use sqlx::Row;
use weasel_core::campaign::{CampaignService, NewCampaign};
use weasel_core::contact::{
    Contact, ContactPatch, ContactQuery, ContactService, ContactSort, NewContact,
};
use weasel_core::enums::{
    CampaignStatus, ContactStatus, ImportScope, ImportStatus, SourceKind,
};
use weasel_core::identity::{IdentityStrategy, NameIdentity};
use weasel_core::ports::{Clipboard, ProgressSink};
use weasel_core::query::PageRequest;
use weasel_core::send::SendService;
use weasel_core::CoreError;
use weasel_import::service::{ImportService, PreviewOptions};
use weasel_import::source::{ParsedSource, RawRow};
use weasel_store::{
    SqliteCampaigns, SqliteContacts, SqliteImports, SqliteSends,
};
use weasel_testkit::assert::assert_fk_clean;
use weasel_testkit::{FakeClipboard, FixedClock, TempDb};

struct Harness {
    db: TempDb,
    clock: Arc<FixedClock>,
    contacts: ContactService,
    imports: ImportService,
    campaigns: CampaignService,
    sends: SendService,
}

impl Harness {
    async fn new() -> Self {
        let db = TempDb::new().await;
        let store = db.store();
        let clock = Arc::new(FixedClock::new(1_700_000_000_000));
        let identity = Arc::new(NameIdentity);
        let contacts = ContactService::new(
            Arc::new(SqliteContacts::new(store.clone())),
            clock.clone(),
            identity.clone(),
        );
        let imports = ImportService::new(
            Arc::new(SqliteImports::new(store.clone())),
            clock.clone(),
            identity.clone(),
        );
        let campaigns = CampaignService::new(
            Arc::new(SqliteCampaigns::new(store.clone())),
            clock.clone(),
        );
        let clipboard: Arc<dyn Clipboard> = Arc::new(FakeClipboard::new());
        let sends = SendService::new(
            Arc::new(SqliteSends::new(store.clone())),
            clipboard,
            clock.clone(),
        );
        Self {
            db,
            clock,
            contacts,
            imports,
            campaigns,
            sends,
        }
    }

    async fn preview(&self, source: &ParsedSource) -> weasel_import::ImportPreview {
        self.imports
            .preview(
                source,
                PreviewOptions {
                    mapping: None,
                    scope: ImportScope::Roster,
                },
                &NoProgress,
            )
            .await
            .unwrap()
    }

    async fn apply(&self, batch_id: i64) -> weasel_core::import::ImportBatch {
        self.imports.apply(batch_id, Vec::new()).await.unwrap()
    }

    async fn all_contacts(&self) -> Vec<Contact> {
        self.contacts
            .list(&ContactQuery {
                search: None,
                statuses: vec![
                    ContactStatus::Active,
                    ContactStatus::Removed,
                    ContactStatus::Archived,
                ],
                import_batch_id: None,
                sort: ContactSort::EmailAsc,
                page: PageRequest {
                    limit: 500,
                    offset: 0,
                },
            })
            .await
            .unwrap()
            .items
    }

    async fn status(&self, id: i64) -> ContactStatus {
        self.contacts.get(id).await.unwrap().status
    }

    async fn by_email(&self, email: &str) -> Contact {
        self.all_contacts()
            .await
            .into_iter()
            .find(|contact| contact.email == email)
            .unwrap()
    }
}

struct NoProgress;

impl ProgressSink for NoProgress {
    fn report(&self, _phase: weasel_core::import::ImportPhase, _processed: u64, _total: u64) {}
}

fn source(rows: &[(&str, &str, &str)]) -> ParsedSource {
    ParsedSource {
        kind: SourceKind::Csv,
        name: "roster.csv".to_owned(),
        path: PathBuf::from("/tmp/roster.csv"),
        file_hash: "c2b7d1e2f0a4".to_owned(),
        sheet_name: None,
        sheets: Vec::new(),
        header_row: 1,
        headers: vec![
            "VORNAME".to_owned(),
            "NACHNAME".to_owned(),
            "EMAIL".to_owned(),
        ],
        rows: rows
            .iter()
            .enumerate()
            .map(|(index, (first, last, email))| RawRow {
                row: index as u32 + 2,
                cells: vec![
                    (*first).to_owned(),
                    (*last).to_owned(),
                    (*email).to_owned(),
                ],
            })
            .collect(),
        ignored_trailing: 0,
    }
}

fn alice() -> (&'static str, &'static str, &'static str) {
    ("Alice", "Smith", "alice@example.edu")
}

fn bob() -> (&'static str, &'static str, &'static str) {
    ("Bob", "Jones", "bob@example.edu")
}

#[tokio::test]
async fn end_to_end_add_previews_then_applies() {
    let h = Harness::new().await;
    let preview = h.preview(&source(&[alice(), bob()])).await;
    assert_eq!(preview.summary.added, 2);
    assert_eq!(preview.summary.conflicts, 0);
    assert_eq!(preview.mapping.first_name, 0);

    let applied = h.apply(preview.batch.id).await;
    assert_eq!(applied.status, ImportStatus::Applied);
    assert!(applied.applied_at.is_some());

    let contacts = h.all_contacts().await;
    assert_eq!(contacts.len(), 2);
    assert!(contacts.iter().all(|contact| contact.source
        == weasel_core::enums::ContactSource::Import
        && contact.import_batch_id == Some(applied.id)));

    let members = h
        .imports
        .members(applied.id, &PageRequest::default())
        .await
        .unwrap();
    assert_eq!(members.total, 2);
    let plan = h
        .imports
        .plan_rows(applied.id, &PageRequest::default())
        .await
        .unwrap();
    assert_eq!(plan.total, 2);
    assert_fk_clean(&h.db.store()).await;
}

#[tokio::test]
async fn re_previewing_the_same_file_changes_nothing() {
    let h = Harness::new().await;
    let preview = h.preview(&source(&[alice(), bob()])).await;
    h.apply(preview.batch.id).await;

    let again = h.preview(&source(&[alice(), bob()])).await;
    assert_eq!(again.summary.added, 0);
    assert_eq!(again.summary.updated, 0);
    assert_eq!(again.summary.removed, 0);
    assert_eq!(again.summary.unchanged, 2);
    assert!(again
        .warnings
        .iter()
        .any(|warning| matches!(
            warning,
            weasel_core::import::PlanWarning::SameFileAsBatch { batch_id }
                if *batch_id == preview.batch.id
        )));
}

#[tokio::test]
async fn changing_an_email_keeps_the_contact_and_closes_history() {
    let h = Harness::new().await;
    let preview = h.preview(&source(&[alice()])).await;
    h.apply(preview.batch.id).await;
    let original = h.by_email("alice@example.edu").await;

    h.clock.advance(1_000);
    let changed = source(&[("Alice", "Smith", "alice.smith@example.edu")]);
    let next = h.preview(&changed).await;
    assert_eq!(next.summary.updated, 1);
    h.apply(next.batch.id).await;

    let updated = h.by_email("alice.smith@example.edu").await;
    assert_eq!(updated.id, original.id);
    assert_eq!(updated.version, original.version + 1);

    let rows = sqlx::query(
        "SELECT email, valid_to FROM contact_email_history WHERE contact_id = ? ORDER BY id",
    )
    .bind(original.id)
    .fetch_all(h.db.store().pool())
    .await
    .unwrap();
    assert_eq!(rows.len(), 2);
    let open = rows
        .iter()
        .filter(|row| row.try_get::<Option<i64>, _>("valid_to").unwrap().is_none())
        .count();
    assert_eq!(open, 1);
    assert_eq!(
        rows[0].try_get::<String, _>("email").unwrap(),
        "alice@example.edu"
    );
}

#[tokio::test]
async fn roster_removal_keeps_send_history_and_a_returnee_is_restored() {
    let h = Harness::new().await;
    let preview = h.preview(&source(&[alice()])).await;
    h.apply(preview.batch.id).await;
    let person = h.by_email("alice@example.edu").await;

    let campaign = h
        .campaigns
        .create(NewCampaign {
            name: "Freshers".to_owned(),
            description: String::new(),
            subject: "Hello".to_owned(),
            body: "Body".to_owned(),
        })
        .await
        .unwrap();
    let active = h
        .campaigns
        .set_status(campaign.id, campaign.version, CampaignStatus::Active)
        .await
        .unwrap();
    h.clock.advance(1_000);
    h.sends.prepare_send(active.id).await.unwrap();

    h.clock.advance(1_000);
    let without_alice = h.preview(&source(&[bob()])).await;
    assert_eq!(without_alice.summary.removed, 1);
    h.apply(without_alice.batch.id).await;
    assert_eq!(h.status(person.id).await, ContactStatus::Removed);

    let sends: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM send_records WHERE contact_id = ?")
        .bind(person.id)
        .fetch_one(h.db.store().pool())
        .await
        .unwrap();
    assert_eq!(sends, 1);

    h.clock.advance(1_000);
    let returned = h.preview(&source(&[alice()])).await;
    assert_eq!(returned.summary.updated, 1);
    h.apply(returned.batch.id).await;
    assert_eq!(h.status(person.id).await, ContactStatus::Active);
    assert_eq!(h.by_email("alice@example.edu").await.id, person.id);
}

#[tokio::test]
async fn unresolved_conflicts_leave_the_draft_untouched() {
    let h = Harness::new().await;
    let manual = h
        .contacts
        .create_manual(NewContact {
            first_name: "Alice".to_owned(),
            last_name: "Smith".to_owned(),
            email: "alice@example.edu".to_owned(),
            notes: String::new(),
        })
        .await
        .unwrap();

    let preview = h.preview(&source(&[("Alicia", "Smith", "alice@example.edu")])).await;
    assert_eq!(preview.conflicts.len(), 1);

    let err = h.imports.apply(preview.batch.id, Vec::new()).await.unwrap_err();
    assert!(matches!(err, CoreError::InvalidState(message) if message.contains("need a decision")));
    let batch = h.imports.get(preview.batch.id).await.unwrap();
    assert_eq!(batch.status, ImportStatus::Draft);
    assert_eq!(h.all_contacts().await.len(), 1);
    assert_eq!(h.status(manual.id).await, ContactStatus::Active);
}

#[tokio::test]
async fn a_stale_preview_is_rejected_without_changes() {
    let h = Harness::new().await;
    let first = h.preview(&source(&[alice()])).await;
    h.apply(first.batch.id).await;
    let person = h.by_email("alice@example.edu").await;

    let next = h.preview(&source(&[("Alice", "Jones", "alice@example.edu")])).await;
    assert_eq!(next.summary.updated, 1);

    h.contacts
        .update(
            person.id,
            ContactPatch {
                version: person.version,
                first_name: Some("Alicia".to_owned()),
                ..ContactPatch::default()
            },
        )
        .await
        .unwrap();

    let err = h.imports.apply(next.batch.id, Vec::new()).await.unwrap_err();
    assert!(
        matches!(err, CoreError::Conflict(message) if message == "Contacts changed since the preview. Preview the file again.")
    );
    assert_eq!(h.by_email("alice@example.edu").await.first_name, "Alicia");
    assert_eq!(
        h.imports.get(next.batch.id).await.unwrap().status,
        ImportStatus::Draft
    );
}

#[tokio::test]
async fn a_mid_transaction_failure_rolls_back_every_row() {
    let h = Harness::new().await;
    let preview = h
        .preview(&source(&[
            alice(),
            bob(),
            ("Carol", "Diaz", "carol@example.edu"),
        ]))
        .await;
    h.contacts
        .create_manual(NewContact {
            first_name: "Carol".to_owned(),
            last_name: "Diaz".to_owned(),
            email: "carol@example.edu".to_owned(),
            notes: String::new(),
        })
        .await
        .unwrap();

    let err = h.imports.apply(preview.batch.id, Vec::new()).await.unwrap_err();
    assert!(matches!(err, CoreError::Conflict(_)));
    let contacts = h.all_contacts().await;
    assert_eq!(contacts.len(), 1);
    assert_eq!(contacts[0].email, "carol@example.edu");
    assert_eq!(
        h.imports.get(preview.batch.id).await.unwrap().status,
        ImportStatus::Draft
    );
    let applied: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM audit_events WHERE action = 'import.apply'")
            .fetch_one(h.db.store().pool())
            .await
            .unwrap();
    assert_eq!(applied, 0);
}

#[tokio::test]
async fn a_batch_cannot_be_applied_twice() {
    let h = Harness::new().await;
    let preview = h.preview(&source(&[alice()])).await;
    h.apply(preview.batch.id).await;
    let err = h.imports.apply(preview.batch.id, Vec::new()).await.unwrap_err();
    assert!(matches!(err, CoreError::InvalidState(_)));
    assert_eq!(h.all_contacts().await.len(), 1);
}

#[tokio::test]
async fn discarding_drops_the_plan_and_leaves_contacts_alone() {
    let h = Harness::new().await;
    let preview = h.preview(&source(&[alice()])).await;
    let discarded = h.imports.discard(preview.batch.id).await.unwrap();
    assert_eq!(discarded.status, ImportStatus::Discarded);
    assert!(h.all_contacts().await.is_empty());

    let plan: Option<String> =
        sqlx::query_scalar("SELECT plan_json FROM import_batches WHERE id = ?")
            .bind(preview.batch.id)
            .fetch_one(h.db.store().pool())
            .await
            .unwrap();
    assert!(plan.is_none());
}

#[tokio::test]
async fn each_operation_and_the_apply_are_audited() {
    let h = Harness::new().await;
    let preview = h.preview(&source(&[alice(), bob()])).await;
    h.apply(preview.batch.id).await;

    let counts = sqlx::query(
        "SELECT action, COUNT(*) AS n FROM audit_events GROUP BY action ORDER BY action",
    )
    .fetch_all(h.db.store().pool())
    .await
    .unwrap();
    let paired: Vec<(String, i64)> = counts
        .iter()
        .map(|row| (row.get("action"), row.get("n")))
        .collect();
    assert_eq!(
        paired,
        vec![
            ("contact.import_create".to_owned(), 2),
            ("import.apply".to_owned(), 1),
            ("import.preview".to_owned(), 1),
        ]
    );
}

#[tokio::test]
async fn a_roster_import_never_removes_manual_contacts() {
    let h = Harness::new().await;
    let manual = h
        .contacts
        .create_manual(NewContact {
            first_name: "Alice".to_owned(),
            last_name: "Smith".to_owned(),
            email: "alice@example.edu".to_owned(),
            notes: String::new(),
        })
        .await
        .unwrap();
    let preview = h.preview(&source(&[bob()])).await;
    assert_eq!(preview.summary.removed, 0);
    h.apply(preview.batch.id).await;
    assert_eq!(h.status(manual.id).await, ContactStatus::Active);
}

#[test]
fn identity_is_the_documented_key() {
    let identity = NameIdentity;
    assert_eq!(identity.id(), "name-v1");
    assert!(identity.key("Alice", "Smith").is_some());
}
