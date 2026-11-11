#![allow(clippy::unwrap_used, clippy::expect_used)]

use weasel_core::{
    CoreError,
    email::EmailAddress,
    enums::{ContactSource, ContactStatus, ImportScope},
    identity::{IdentityStrategy, NAME_IDENTITY_ID, NameIdentity},
    import::{
        ColumnMapping, ConflictAction, ConflictKind, ConflictResolution, ContactOp, ContactRef,
        ImportPlan, NormalizedRow, PlanAction, PlanWarning, ReconcileSnapshot, RowError,
    },
    ports::{NoProgress, ProgressSink},
};
use weasel_import::reconcile::{action_for, plan, resolve};

fn email(raw: &str) -> EmailAddress {
    EmailAddress::parse(raw).unwrap()
}

fn contact(
    id: i64,
    first: &str,
    last: &str,
    address: &str,
    status: ContactStatus,
    source: ContactSource,
) -> ContactRef {
    ContactRef {
        id,
        version: 3,
        first_name: first.to_owned(),
        last_name: last.to_owned(),
        email: address.to_owned(),
        email_normalized: address.to_lowercase(),
        identity_key: NameIdentity.key(first, last),
        status,
        source,
    }
}

fn row(number: u32, first: &str, last: &str, address: &str) -> NormalizedRow {
    NormalizedRow {
        row: number,
        first_name: first.to_owned(),
        last_name: last.to_owned(),
        email: email(address),
        identity_key: NameIdentity.key(first, last),
    }
}

fn snapshot(contacts: Vec<ContactRef>, history: &[(&str, i64)]) -> ReconcileSnapshot {
    ReconcileSnapshot {
        contacts,
        history: history
            .iter()
            .map(|(address, id)| ((*address).to_owned(), *id))
            .collect(),
    }
}

fn planning(
    rows: &[NormalizedRow],
    errors: &[RowError],
    snapshot: &ReconcileSnapshot,
    scope: ImportScope,
) -> ImportPlan {
    plan(
        rows,
        errors,
        snapshot,
        scope,
        &mapping(),
        NAME_IDENTITY_ID,
        &NoProgress,
    )
    .unwrap()
}

fn mapping() -> ColumnMapping {
    ColumnMapping {
        first_name: 0,
        last_name: 1,
        email: 2,
    }
}

fn action(plan: &ImportPlan, row_number: u32) -> Action {
    let planned = plan
        .rows
        .iter()
        .find(|planned| planned.input.row == row_number)
        .unwrap();
    match &planned.action {
        PlanAction::Add => Action::Add,
        PlanAction::UpdateDetails { .. } => Action::Details,
        PlanAction::UpdateEmail { .. } => Action::Email,
        PlanAction::Restore { .. } => Action::Restore,
        PlanAction::Noop { .. } => Action::Noop,
        PlanAction::Conflict { index } => Action::Conflict(*index),
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Action {
    Add,
    Details,
    Email,
    Restore,
    Noop,
    Conflict(usize),
}

#[test]
fn new_same_names_and_email_are_classified() {
    let ada = contact(
        1,
        "Ada",
        "Lovelace",
        "ada@example.edu",
        ContactStatus::Active,
        ContactSource::Import,
    );
    let bob = contact(
        2,
        "Bob",
        "Babbage",
        "bob@example.edu",
        ContactStatus::Active,
        ContactSource::Import,
    );
    let cleo = contact(
        3,
        "Cleo",
        "Curtis",
        "cleo@example.edu",
        ContactStatus::Active,
        ContactSource::Import,
    );
    let dora = contact(
        4,
        "Dora",
        "Dee",
        "dora@example.edu",
        ContactStatus::Active,
        ContactSource::Import,
    );
    let erin = contact(
        5,
        "Erin",
        "Eve",
        "erin@example.edu",
        ContactStatus::Active,
        ContactSource::Import,
    );
    let rows = vec![
        row(2, "Ada", "Lovelace", "ada@example.edu"),
        row(3, "Ada Lovelace", "Lovelace", "ada@example.edu"),
        row(4, "Ada", "Lovelace", "ada@example.edu"),
        row(5, "Bobbie", "Babbage", "bob@example.edu"),
        row(6, "Cleo", "Curtis", "cleo.new@example.edu"),
        row(7, "Dora", "Dee", "dora@example.edu"),
        row(8, "Erin", "Eve", "erin.new@example.edu"),
        row(9, "New", "Person", "new@example.edu"),
    ];
    let snap = snapshot(
        vec![ada, bob, cleo, dora, erin],
        &[("cleo.new@example.edu", 3)],
    );
    let plan = planning(&rows, &[], &snap, ImportScope::Delta);

    // Duplicate source rows for Ada collapse into one conflict before matching.
    assert_eq!(action(&plan, 2), Action::Conflict(0));
    assert_eq!(action(&plan, 3), Action::Conflict(0));
    assert_eq!(action(&plan, 4), Action::Conflict(0));
    assert_eq!(plan.conflicts[0].kind, ConflictKind::DuplicateEmailInSource);
    assert_eq!(plan.conflicts[0].candidates.len(), 1);

    assert_eq!(action(&plan, 5), Action::Details);
    assert_eq!(action(&plan, 6), Action::Email); // found through history
    assert_eq!(action(&plan, 7), Action::Noop);
    assert_eq!(action(&plan, 8), Action::Email); // found through the name key
    assert_eq!(action(&plan, 9), Action::Add);
    assert_eq!(plan.summary.added, 1);
    assert_eq!(plan.summary.updated, 3);
    assert_eq!(plan.summary.unchanged, 1);
    assert_eq!(plan.summary.conflicts, 1);
}

#[test]
fn identity_and_history_ambiguity_are_conflicts() {
    let first = contact(
        1,
        "Ada",
        "Lovelace",
        "one@example.edu",
        ContactStatus::Active,
        ContactSource::Import,
    );
    let mut second = contact(
        2,
        "Ada",
        "Lovelace",
        "two@example.edu",
        ContactStatus::Active,
        ContactSource::Import,
    );
    second.identity_key = NameIdentity.key("Ada", "Lovelace");
    let rows = vec![
        row(2, "Ada", "Lovelace", "three@example.edu"),
        row(3, "Someone", "Else", "old@example.edu"),
    ];
    let snap = snapshot(
        vec![first, second],
        &[("old@example.edu", 1), ("old@example.edu", 2)],
    );
    // Row 3 finds history for old@example.edu pointing at two different people.
    let plan = planning(&rows, &[], &snap, ImportScope::Delta);
    assert_eq!(action(&plan, 2), Action::Conflict(0));
    assert_eq!(plan.conflicts[0].kind, ConflictKind::AmbiguousIdentity);
    assert_eq!(plan.conflicts[0].candidates.len(), 2);
    assert_eq!(action(&plan, 3), Action::Conflict(1));
    assert_eq!(plan.conflicts[1].kind, ConflictKind::AmbiguousHistory);
    assert_eq!(plan.conflicts[1].candidates.len(), 2);
}

#[test]
fn removed_people_restore_and_identity_beats_a_new_address() {
    let gone = contact(
        1,
        "Ada",
        "Lovelace",
        "old@example.edu",
        ContactStatus::Removed,
        ContactSource::Import,
    );
    let moved = contact(
        2,
        "Grace",
        "Hopper",
        "grace.old@example.edu",
        ContactStatus::Active,
        ContactSource::Import,
    );
    let rows = vec![
        row(4, "Ada", "Lovelace", "old@example.edu"),
        row(5, "Grace", "Hopper", "grace.new@example.edu"),
    ];
    let plan = planning(
        &rows,
        &[],
        &snapshot(vec![gone, moved], &[]),
        ImportScope::Delta,
    );
    assert_eq!(action(&plan, 4), Action::Restore);
    assert_eq!(action(&plan, 5), Action::Email);
    assert_eq!(plan.summary.updated, 2);
}

#[test]
fn rosters_remove_absent_import_contacts_but_deltas_do_not() {
    let absent = contact(
        1,
        "Absent",
        "Imported",
        "absent@example.edu",
        ContactStatus::Active,
        ContactSource::Import,
    );
    let manual = contact(
        2,
        "Manual",
        "Person",
        "manual@example.edu",
        ContactStatus::Active,
        ContactSource::Manual,
    );
    let archived = contact(
        3,
        "Archived",
        "Person",
        "archived@example.edu",
        ContactStatus::Archived,
        ContactSource::Import,
    );
    let removed = contact(
        4,
        "Removed",
        "Person",
        "removed@example.edu",
        ContactStatus::Removed,
        ContactSource::Import,
    );
    let present = contact(
        5,
        "Present",
        "Person",
        "present@example.edu",
        ContactStatus::Active,
        ContactSource::Import,
    );
    let rows = vec![row(2, "Present", "Person", "present@example.edu")];

    let roster = planning(
        &rows,
        &[],
        &snapshot(
            vec![absent, manual, archived, removed, present.clone()],
            &[],
        ),
        ImportScope::Roster,
    );
    assert_eq!(roster.removals.len(), 1);
    assert_eq!(roster.removals[0].contact.id, 1);
    assert_eq!(action(&roster, 2), Action::Noop);

    let delta = planning(
        &rows,
        &[],
        &snapshot(vec![present], &[]),
        ImportScope::Delta,
    );
    assert!(delta.removals.is_empty());
}

#[test]
fn manual_contacts_require_a_decision() {
    let manual = contact(
        1,
        "Manual",
        "Person",
        "manual@example.edu",
        ContactStatus::Active,
        ContactSource::Manual,
    );
    let rows = vec![row(2, "Manual", "Person", "manual@example.edu")];
    let plan = planning(&rows, &[], &snapshot(vec![manual], &[]), ImportScope::Delta);
    assert_eq!(action(&plan, 2), Action::Conflict(0));
    assert_eq!(plan.conflicts[0].kind, ConflictKind::ManualContact);
    assert_eq!(plan.conflicts[0].candidates[0].id, 1);
}

#[test]
fn one_person_cannot_be_matched_by_two_rows() {
    let ada = contact(
        1,
        "Ada",
        "Lovelace",
        "ada@example.edu",
        ContactStatus::Active,
        ContactSource::Import,
    );
    let rows = vec![
        row(2, "Ada", "Byron", "ada@example.edu"),
        row(3, "Ada", "Lovelace", "ada.new@example.edu"),
    ];
    let plan = planning(&rows, &[], &snapshot(vec![ada], &[]), ImportScope::Delta);
    assert_eq!(action(&plan, 2), Action::Conflict(0));
    assert_eq!(action(&plan, 3), Action::Conflict(0));
    assert_eq!(plan.conflicts[0].kind, ConflictKind::ContactMatchedTwice);
    assert_eq!(plan.conflicts[0].rows, vec![2, 3]);
}

#[test]
fn invalid_rows_and_conflict_candidates_are_not_removed() {
    let protected = contact(
        1,
        "Protected",
        "Person",
        "protected@example.edu",
        ContactStatus::Active,
        ContactSource::Import,
    );
    let candidate = contact(
        2,
        "Candidate",
        "Person",
        "dup@example.edu",
        ContactStatus::Active,
        ContactSource::Import,
    );
    let disposable = contact(
        3,
        "Disposable",
        "Person",
        "gone@example.edu",
        ContactStatus::Active,
        ContactSource::Import,
    );
    let rows = vec![
        row(2, "New", "Person", "new@example.edu"),
        row(3, "Candidate", "Person", "dup@example.edu"),
        row(4, "Candidate", "Person", "dup@example.edu"),
    ];
    let errors = vec![RowError {
        row: 5,
        field: "lastName".to_owned(),
        reason: "Required".to_owned(),
        identity_key: protected.identity_key.clone(),
    }];
    let plan = planning(
        &rows,
        &errors,
        &snapshot(vec![protected, candidate, disposable], &[]),
        ImportScope::Roster,
    );
    assert_eq!(plan.removals.len(), 1);
    assert_eq!(plan.removals[0].contact.id, 3);
    assert_eq!(plan.summary.invalid, 1);
    assert_eq!(action(&plan, 3), Action::Conflict(0));
    assert_eq!(action(&plan, 4), Action::Conflict(0));
}

#[test]
fn archived_people_are_added_again() {
    let archived = contact(
        1,
        "Ada",
        "Lovelace",
        "ada@example.edu",
        ContactStatus::Archived,
        ContactSource::Import,
    );
    let rows = vec![row(2, "Ada", "Lovelace", "ada@example.edu")];
    let plan = planning(
        &rows,
        &[],
        &snapshot(vec![archived], &[]),
        ImportScope::Roster,
    );
    assert_eq!(action(&plan, 2), Action::Add);
    assert!(plan.removals.is_empty());
}

#[test]
fn empty_rosters_fail_and_large_removals_warn() {
    let empty = snapshot(vec![], &[]);
    let error = plan(
        &[],
        &[],
        &empty,
        ImportScope::Roster,
        &mapping(),
        NAME_IDENTITY_ID,
        &NoProgress,
    )
    .unwrap_err();
    assert!(matches!(
        error,
        CoreError::Validation {
            field: "source",
            ..
        }
    ));

    let contacts = (1..=10)
        .map(|id| {
            contact(
                id,
                &format!("Person{id}"),
                "Absent",
                &format!("person{id}@example.edu"),
                ContactStatus::Active,
                ContactSource::Import,
            )
        })
        .collect();
    let rows = vec![row(2, "Only", "Newcomer", "only@example.edu")];
    let plan = planning(&rows, &[], &snapshot(contacts, &[]), ImportScope::Roster);
    assert_eq!(plan.removals.len(), 10);
    assert!(matches!(
        plan.warnings.as_slice(),
        [PlanWarning::LargeRemoval {
            count: 10,
            active_import_contacts: 10
        }]
    ));
}

#[test]
fn resolutions_become_operations_and_reject_bad_answers() {
    let rows = vec![
        row(2, "Duplicate", "Person", "dup@example.edu"),
        row(3, "Duplicate", "Person", "dup@example.edu"),
    ];
    let snap = snapshot(vec![], &[]);
    let plan = planning(&rows, &[], &snap, ImportScope::Delta);

    assert!(matches!(
        resolve(&plan, &[]).unwrap_err(),
        CoreError::InvalidState(message) if message == "2 rows need a decision"
    ));
    assert!(matches!(
        resolve(
            &plan,
            &[ConflictResolution {
                row: 99,
                action: ConflictAction::Skip,
            }]
        )
        .unwrap_err(),
        CoreError::Validation {
            field: "resolutions",
            ..
        }
    ));
    assert!(matches!(
        resolve(
            &plan,
            &[
                ConflictResolution {
                    row: 2,
                    action: ConflictAction::CreateNew,
                },
                ConflictResolution {
                    row: 3,
                    action: ConflictAction::CreateNew,
                },
            ]
        )
        .unwrap_err(),
        CoreError::Validation {
            field: "resolutions",
            ..
        }
    ));

    let ops = resolve(
        &plan,
        &[
            ConflictResolution {
                row: 2,
                action: ConflictAction::CreateNew,
            },
            ConflictResolution {
                row: 3,
                action: ConflictAction::Skip,
            },
        ],
    )
    .unwrap();
    assert_eq!(ops.len(), 1);
    assert!(matches!(
        &ops[0],
        ContactOp::Create { row: 2, email, .. } if email.normalized == "dup@example.edu"
    ));

    let manual = contact(
        1,
        "Manual",
        "Person",
        "manual@example.edu",
        ContactStatus::Active,
        ContactSource::Manual,
    );
    let rows = vec![row(2, "Manual", "Changed", "manual@example.edu")];
    let manual_plan = planning(&rows, &[], &snapshot(vec![manual], &[]), ImportScope::Delta);
    assert!(matches!(
        resolve(
            &manual_plan,
            &[ConflictResolution {
                row: 2,
                action: ConflictAction::LinkContact { contact_id: 99 },
            }]
        )
        .unwrap_err(),
        CoreError::Validation {
            field: "resolutions",
            ..
        }
    ));
    assert!(matches!(
        resolve(
            &manual_plan,
            &[ConflictResolution {
                row: 2,
                action: ConflictAction::CreateNew,
            }]
        )
        .unwrap_err(),
        CoreError::Validation {
            field: "resolutions",
            ..
        }
    ));
    let ops = resolve(
        &manual_plan,
        &[ConflictResolution {
            row: 2,
            action: ConflictAction::LinkContact { contact_id: 1 },
        }],
    )
    .unwrap();
    assert!(matches!(
        &ops[0],
        ContactOp::Modify {
            contact_id: 1,
            last_name: Some(last),
            expected_version: 3,
            status: None,
            ..
        } if last == "Changed"
    ));
    assert!(
        resolve(
            &manual_plan,
            &[ConflictResolution {
                row: 2,
                action: ConflictAction::Skip,
            }]
        )
        .unwrap()
        .is_empty()
    );
}

#[test]
fn linking_two_rows_to_one_person_is_rejected() {
    let ada = contact(
        1,
        "Ada",
        "Lovelace",
        "ada@example.edu",
        ContactStatus::Active,
        ContactSource::Import,
    );
    let rows = vec![
        row(2, "Ada", "Byron", "ada@example.edu"),
        row(3, "Ada", "Lovelace", "ada.new@example.edu"),
    ];
    let plan = planning(&rows, &[], &snapshot(vec![ada], &[]), ImportScope::Delta);
    let answer = ConflictResolution {
        row: 2,
        action: ConflictAction::LinkContact { contact_id: 1 },
    };
    let second = ConflictResolution {
        row: 3,
        action: ConflictAction::LinkContact { contact_id: 1 },
    };
    assert!(matches!(
        resolve(&plan, &[answer, second]).unwrap_err(),
        CoreError::InvalidState(message) if message.contains("twice")
    ));
}

#[test]
fn action_for_compares_cleaned_names() {
    let ada = contact(
        1,
        "Ada",
        "Lovelace",
        "ada@example.edu",
        ContactStatus::Active,
        ContactSource::Import,
    );
    assert!(matches!(
        action_for(&row(2, "  Ada ", "Lovelace", "ada@example.edu"), &ada),
        PlanAction::Noop { contact_id: 1 }
    ));
}

#[test]
fn progress_is_reported_for_reading_and_matching() {
    #[derive(Default)]
    struct Recorder(std::sync::Mutex<Vec<(weasel_core::import::ImportPhase, u64, u64)>>);
    impl ProgressSink for Recorder {
        fn report(&self, phase: weasel_core::import::ImportPhase, processed: u64, total: u64) {
            self.0.lock().unwrap().push((phase, processed, total));
        }
    }
    let recorder = Recorder::default();
    let rows: Vec<_> = (0..501)
        .map(|index| {
            row(
                index + 2,
                "First",
                &format!("Last{index}"),
                &format!("p{index}@example.edu"),
            )
        })
        .collect();
    plan(
        &rows,
        &[],
        &snapshot(vec![], &[]),
        ImportScope::Delta,
        &mapping(),
        NAME_IDENTITY_ID,
        &recorder,
    )
    .unwrap();
    let calls = recorder.0.lock().unwrap();
    assert!(
        calls
            .iter()
            .all(|(phase, _, _)| *phase == weasel_core::import::ImportPhase::Matching)
    );
    assert!(calls.iter().any(|(_, processed, _)| *processed == 0));
    assert!(calls.iter().any(|(_, processed, _)| *processed == 500));
    assert_eq!(calls.last().unwrap().1, 501);
}
