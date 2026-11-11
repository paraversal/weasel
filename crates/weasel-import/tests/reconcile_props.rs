#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::HashSet;

use proptest::prelude::*;
use weasel_core::{
    email::EmailAddress,
    enums::{ContactSource, ContactStatus, ImportScope},
    identity::{IdentityStrategy, NameIdentity, NAME_IDENTITY_ID},
    import::{
        ColumnMapping, ConflictAction, ConflictResolution, ContactOp, ContactRef,
        ImportPlan, NormalizedRow, PlanAction, ReconcileSnapshot,
    },
    ports::NoProgress,
};
use weasel_import::reconcile::{plan, resolve};

const MAPPING: ColumnMapping = ColumnMapping {
    first_name: 0,
    last_name: 1,
    email: 2,
};

fn contact(id: i64, status: ContactStatus, source: ContactSource) -> ContactRef {
    let first = format!("First{id}");
    let last = format!("Last{id}");
    ContactRef {
        id,
        version: 2,
        identity_key: id_key(&first, &last),
        first_name: first,
        last_name: last,
        email: format!("c{id}@example.edu"),
        email_normalized: format!("c{id}@example.edu"),
        status,
        source,
    }
}

fn id_key(first: &str, last: &str) -> Option<String> {
    NameIdentity.key(first, last)
}

fn person(row: u32, contact_id: i64) -> NormalizedRow {
    NormalizedRow {
        row,
        first_name: format!("First{contact_id}"),
        last_name: format!("Last{contact_id}"),
        email: EmailAddress::parse(&format!("c{contact_id}@example.edu")).unwrap(),
        identity_key: id_key(&format!("First{contact_id}"), &format!("Last{contact_id}")),
    }
}

fn newcomer(row: u32) -> NormalizedRow {
    let first = format!("New{row}");
    let last = format!("Person{row}");
    NormalizedRow {
        row,
        first_name: first.clone(),
        last_name: last.clone(),
        email: EmailAddress::parse(&format!("new{row}@example.edu")).unwrap(),
        identity_key: id_key(&first, &last),
    }
}

fn old_address(row: u32, contact_id: i64) -> NormalizedRow {
    NormalizedRow {
        row,
        first_name: format!("First{contact_id}"),
        last_name: format!("Last{contact_id}"),
        email: EmailAddress::parse(&format!("old{contact_id}@example.edu")).unwrap(),
        identity_key: id_key(&format!("First{contact_id}"), &format!("Last{contact_id}")),
    }
}

fn snapshot(contacts: &[ContactRef], history: &[(String, i64)]) -> ReconcileSnapshot {
    ReconcileSnapshot {
        contacts: contacts.to_vec(),
        history: history.to_vec(),
    }
}

fn plan_delta(rows: &[NormalizedRow], snapshot: &ReconcileSnapshot) -> ImportPlan {
    plan(
        rows,
        &[],
        snapshot,
        ImportScope::Delta,
        &MAPPING,
        NAME_IDENTITY_ID,
        &NoProgress,
    )
    .unwrap()
}

fn target(action: &PlanAction) -> Option<i64> {
    match action {
        PlanAction::UpdateDetails { target }
        | PlanAction::UpdateEmail { target }
        | PlanAction::Restore { target } => Some(target.id),
        PlanAction::Noop { contact_id } => Some(*contact_id),
        PlanAction::Add | PlanAction::Conflict { .. } => None,
    }
}

fn apply(mut contacts: Vec<ContactRef>, ops: &[ContactOp]) -> Vec<ContactRef> {
    let mut next_id = contacts.iter().map(|contact| contact.id).max().unwrap_or(0) + 1;
    for op in ops {
        match op {
            ContactOp::Create {
                first_name,
                last_name,
                email,
                identity_key,
                ..
            } => {
                contacts.push(ContactRef {
                    id: next_id,
                    version: 1,
                    first_name: first_name.clone(),
                    last_name: last_name.clone(),
                    email: email.display.clone(),
                    email_normalized: email.normalized.clone(),
                    identity_key: identity_key.clone(),
                    status: ContactStatus::Active,
                    source: ContactSource::Import,
                });
                next_id += 1;
            }
            ContactOp::Modify {
                contact_id,
                expected_version,
                first_name,
                last_name,
                email,
                status,
                identity_key,
                ..
            } => {
                let contact = contacts
                    .iter_mut()
                    .find(|contact| contact.id == *contact_id)
                    .unwrap();
                assert_eq!(contact.version, *expected_version);
                if let Some(first) = first_name {
                    contact.first_name = first.clone();
                }
                if let Some(last) = last_name {
                    contact.last_name = last.clone();
                }
                if let Some(email) = email {
                    contact.email = email.display.clone();
                    contact.email_normalized = email.normalized.clone();
                }
                if let Some(status) = status {
                    contact.status = *status;
                }
                if let Some(key) = identity_key {
                    contact.identity_key = Some(key.clone());
                }
                contact.version += 1;
            }
        }
    }
    contacts
}

fn assert_unique_live_emails(contacts: &[ContactRef]) {
    let mut seen = HashSet::new();
    for contact in contacts {
        if contact.status != ContactStatus::Archived {
            assert!(
                seen.insert(contact.email_normalized.as_str()),
                "two live contacts share {}",
                contact.email_normalized
            );
        }
    }
}

#[derive(Debug, Clone)]
enum Pick {
    Existing(u8),
    Old(u8),
    New,
}

fn pick() -> impl Strategy<Value = Pick> {
    prop_oneof![
        3 => (0u8..8).prop_map(Pick::Existing),
        1 => (0u8..8).prop_map(Pick::Old),
        2 => Just(Pick::New),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(96))]

    /// Every input row is accounted for exactly once, no person is targeted twice,
    /// and summaries agree with the plan.
    #[test]
    fn plans_account_for_every_row_and_every_target(
        contacts in prop::collection::vec(
            (any::<prop::sample::Index>(), any::<prop::sample::Index>()),
            0..8,
        ),
        row_count in 1usize..25,
        picks in prop::collection::vec(pick(), 1..25),
    ) {
        let contacts: Vec<ContactRef> = contacts
            .iter()
            .enumerate()
            .map(|(index, _)| {
                let status = [ContactStatus::Active, ContactStatus::Removed, ContactStatus::Archived]
                    [index % 3];
                let source = [ContactSource::Import, ContactSource::Manual][index % 2];
                contact(index as i64 + 1, status, source)
            })
            .collect();
        let history: Vec<(String, i64)> = contacts
            .iter()
            .map(|contact| (format!("old{}@example.edu", contact.id), contact.id))
            .collect();
        let snap = snapshot(&contacts, &history);

        let rows: Vec<NormalizedRow> = (0..row_count)
            .map(|index| {
                let row = index as u32 + 2;
                match picks[index % picks.len()] {
                    Pick::Existing(id) if (id as i64) < contacts.len() as i64 => person(row, id as i64 + 1),
                    Pick::Old(id) if (id as i64) < contacts.len() as i64 => old_address(row, id as i64 + 1),
                    _ => newcomer(row),
                }
            })
            .collect();

        let plan = plan_delta(&rows, &snap);

        prop_assert_eq!(plan.rows.len(), rows.len());
        let mut seen = HashSet::new();
        let mut targets = HashSet::new();
        for (index, planned) in plan.rows.iter().enumerate() {
            prop_assert_eq!(planned.input.row, rows[index].row);
            prop_assert!(seen.insert(planned.input.row));
            if let PlanAction::Conflict { index } = planned.action {
                prop_assert!(index < plan.conflicts.len());
            }
            if let Some(id) = target(&planned.action) {
                prop_assert!(targets.insert(id));
            }
        }
        for removal in &plan.removals {
            prop_assert_eq!(removal.contact.status, ContactStatus::Active);
            prop_assert_eq!(removal.contact.source, ContactSource::Import);
            prop_assert!(targets.insert(removal.contact.id));
        }
        for conflict in &plan.conflicts {
            prop_assert!(!conflict.rows.is_empty());
        }

        let mut added = 0;
        let mut updated = 0;
        let mut unchanged = 0;
        for planned in &plan.rows {
            match planned.action {
                PlanAction::Add => added += 1,
                PlanAction::UpdateDetails { .. }
                | PlanAction::UpdateEmail { .. }
                | PlanAction::Restore { .. } => updated += 1,
                PlanAction::Noop { .. } => unchanged += 1,
                PlanAction::Conflict { .. } => {}
            }
        }
        prop_assert_eq!(plan.summary.row_count, rows.len() as u64);
        prop_assert_eq!(plan.summary.added, added);
        prop_assert_eq!(plan.summary.updated, updated);
        prop_assert_eq!(plan.summary.unchanged, unchanged);
        prop_assert_eq!(plan.summary.removed, plan.removals.len() as u64);
        prop_assert_eq!(plan.summary.conflicts, plan.conflicts.len() as u64);
        prop_assert!(plan.removals.is_empty(), "delta plans never remove");
    }

    /// With every conflict skipped, applying a plan never creates a duplicate live
    /// address, and re-planning the result is a no-op.
    #[test]
    fn applying_a_plan_preserves_unique_live_addresses(
        contacts in prop::collection::vec(
            (any::<prop::sample::Index>(), any::<prop::sample::Index>()),
            0..8,
        ),
        row_count in 1usize..25,
        picks in prop::collection::vec(pick(), 1..25),
        pass in 0u8..2,
    ) {
        let contacts: Vec<ContactRef> = contacts
            .iter()
            .enumerate()
            .map(|(index, _)| {
                let status = [ContactStatus::Active, ContactStatus::Removed, ContactStatus::Archived]
                    [index % 3];
                let source = if pass == 0 {
                    ContactSource::Import
                } else {
                    [ContactSource::Import, ContactSource::Manual][index % 2]
                };
                contact(index as i64 + 1, status, source)
            })
            .collect();
        let history: Vec<(String, i64)> = contacts
            .iter()
            .map(|contact| (format!("old{}@example.edu", contact.id), contact.id))
            .collect();
        let snap = snapshot(&contacts, &history);
        let rows: Vec<NormalizedRow> = (0..row_count)
            .map(|index| {
                let row = index as u32 + 2;
                match picks[index % picks.len()] {
                    Pick::Existing(id) if (id as i64) < contacts.len() as i64 => person(row, id as i64 + 1),
                    Pick::Old(id) if (id as i64) < contacts.len() as i64 => old_address(row, id as i64 + 1),
                    _ => newcomer(row),
                }
            })
            .collect();

        let plan = plan_delta(&rows, &snap);
        let mut used: HashSet<i64> = plan
            .rows
            .iter()
            .filter_map(|planned| target(&planned.action))
            .chain(plan.removals.iter().map(|removal| removal.contact.id))
            .collect();
        let mut resolutions = Vec::new();
        for conflict in &plan.conflicts {
            for row in &conflict.rows {
                let contact_id = (conflict.rows.len() == 1 && conflict.candidates.len() == 1)
                    .then(|| conflict.candidates[0].id)
                    .filter(|contact_id| used.insert(*contact_id));
                resolutions.push(ConflictResolution {
                    row: *row,
                    action: contact_id.map_or(ConflictAction::Skip, |contact_id| {
                        ConflictAction::LinkContact { contact_id }
                    }),
                });
            }
        }
        let ops = resolve(&plan, &resolutions).unwrap();
        let applied = apply(contacts.clone(), &ops);
        assert_unique_live_emails(&applied);

        if plan.conflicts.is_empty() {
            let replanned = plan_delta(&rows, &snapshot(&applied, &[]));
            prop_assert!(replanned.conflicts.is_empty());
            prop_assert!(replanned.removals.is_empty());
            for planned in &replanned.rows {
                prop_assert!(
                    matches!(planned.action, PlanAction::Noop { .. }),
                    "re-planning produced {:?} for row {}",
                    planned.action,
                    planned.input.row
                );
            }
        }
    }

    /// Confirmed plans re-plan to no-ops once applied.
    #[test]
    fn a_confirmed_plan_replans_as_noops(
        statuses in prop::collection::vec(0u8..3, 1..9),
        newcomers in 0usize..8,
    ) {
        let contacts: Vec<ContactRef> = statuses
            .iter()
            .enumerate()
            .map(|(index, status)| {
                let status = [ContactStatus::Active, ContactStatus::Removed, ContactStatus::Archived]
                    [(*status as usize) % 3];
                contact(index as i64 + 1, status, ContactSource::Import)
            })
            .collect();
        let mut rows: Vec<NormalizedRow> = (0..statuses.len())
            .map(|index| person(index as u32 + 2, index as i64 + 1))
            .collect();
        for index in 0..newcomers {
            rows.push(newcomer(rows.len() as u32 + 2 + index as u32));
        }

        let plan = plan_delta(&rows, &snapshot(&contacts, &[]));
        prop_assert!(plan.conflicts.is_empty());
        prop_assert!(plan.removals.is_empty());
        let ops = resolve(&plan, &[]).unwrap();
        let applied = apply(contacts, &ops);
        assert_unique_live_emails(&applied);

        let replanned = plan_delta(&rows, &snapshot(&applied, &[]));
        prop_assert!(replanned.conflicts.is_empty());
        prop_assert!(replanned.removals.is_empty());
        for planned in &replanned.rows {
            prop_assert!(
                matches!(planned.action, PlanAction::Noop { .. }),
                "re-planning produced {:?} for row {}",
                planned.action,
                planned.input.row
            );
        }
    }
}
