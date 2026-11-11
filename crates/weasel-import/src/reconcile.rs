//! Pure reconciliation: sheet rows against a snapshot of the address book.
//!
//! No I/O, no clock. The planner decides what *would* happen; [`resolve`] turns
//! the user's decisions into ordered, atomic operations.

use std::collections::{HashMap, HashSet};

use weasel_core::{
    CoreError, CoreResult,
    enums::{ContactSource, ContactStatus, ImportScope},
    identity::clean_name,
    import::{
        ColumnMapping, Conflict, ConflictAction, ConflictKind, ConflictResolution, ContactOp,
        ContactRef, ImportPhase, ImportPlan, ImportSummary, NormalizedRow, PLAN_VERSION,
        PlanAction, PlanWarning, PlannedRemoval, PlannedRow, ReconcileSnapshot, RowError,
    },
    ports::ProgressSink,
};

#[derive(Debug, Clone)]
enum Outcome {
    Action(PlanAction),
    Conflict(usize),
}

/// Plan one import.
///
/// `rows` and `errors` come from [`crate::normalize::normalize`]. Every valid row
/// appears exactly once in `plan.rows`; a conflicting row carries
/// [`PlanAction::Conflict`] with the index into `plan.conflicts`.
#[allow(clippy::too_many_arguments)]
pub fn plan(
    rows: &[NormalizedRow],
    errors: &[RowError],
    snapshot: &ReconcileSnapshot,
    scope: ImportScope,
    mapping: &ColumnMapping,
    strategy_id: &str,
    progress: &dyn ProgressSink,
) -> CoreResult<ImportPlan> {
    if rows.is_empty() {
        return Err(CoreError::Validation {
            field: "source",
            reason: "No valid rows; a roster import would remove everyone.".to_owned(),
        });
    }

    let contacts: Vec<&ContactRef> = snapshot
        .contacts
        .iter()
        .filter(|contact| contact.status != ContactStatus::Archived)
        .collect();
    let by_id: HashMap<i64, &ContactRef> = contacts
        .iter()
        .map(|contact| (contact.id, *contact))
        .collect();
    let live_by_email = index_by(&contacts, |contact| contact.email_normalized.clone());
    let by_key = index_by(&contacts, |contact| {
        contact.identity_key.clone().unwrap_or_default()
    });
    let history_by_email = history_index(snapshot, &by_id);

    let mut conflicts: Vec<Conflict> = Vec::new();
    let mut outcomes: Vec<Outcome> = vec![Outcome::Action(PlanAction::Add); rows.len()];
    let mut conflicted_rows: HashSet<u32> = HashSet::new();

    // 1. Duplicate addresses inside the file.
    let mut by_email: HashMap<&str, Vec<usize>> = HashMap::new();
    for (index, row) in rows.iter().enumerate() {
        by_email
            .entry(row.email.normalized.as_str())
            .or_default()
            .push(index);
    }
    for (email, indexes) in &by_email {
        if indexes.len() < 2 {
            continue;
        }
        let candidates: Vec<ContactRef> = live_by_email
            .get(*email)
            .map(|contacts| contacts.iter().map(|contact| (*contact).clone()).collect())
            .unwrap_or_default();
        let conflict_index = push_conflict(
            &mut conflicts,
            Conflict {
                rows: indexes.iter().map(|index| rows[*index].row).collect(),
                kind: ConflictKind::DuplicateEmailInSource,
                candidates,
            },
        );
        for index in indexes {
            conflicted_rows.insert(rows[*index].row);
            outcomes[*index] = Outcome::Conflict(conflict_index);
        }
    }

    // 2-3. Match each remaining row by current email, then history, then name key.
    let total = rows.len() as u64;
    for (index, row) in rows.iter().enumerate() {
        if (index as u64).is_multiple_of(500) {
            progress.report(ImportPhase::Matching, index as u64, total);
        }
        if conflicted_rows.contains(&row.row) {
            continue;
        }

        let candidate = if let Some(current) = live_by_email.get(&row.email.normalized) {
            current.first().map(|contact| (*contact).clone())
        } else if let Some(history) = history_by_email.get(row.email.normalized.as_str()) {
            match distinct(history) {
                0 => None,
                1 => history
                    .first()
                    .and_then(|id| by_id.get(id))
                    .map(|contact| (*contact).clone()),
                _ => {
                    let conflict_index = push_conflict(
                        &mut conflicts,
                        Conflict {
                            rows: vec![row.row],
                            kind: ConflictKind::AmbiguousHistory,
                            candidates: distinct_refs(history, &by_id),
                        },
                    );
                    outcomes[index] = Outcome::Conflict(conflict_index);
                    continue;
                }
            }
        } else if let Some(key) = &row.identity_key {
            match by_key.get(key.as_str()) {
                Some(contacts) if contacts.len() == 1 => Some((*contacts[0]).clone()),
                Some(contacts) => {
                    let conflict_index = push_conflict(
                        &mut conflicts,
                        Conflict {
                            rows: vec![row.row],
                            kind: ConflictKind::AmbiguousIdentity,
                            candidates: contacts.iter().map(|contact| (*contact).clone()).collect(),
                        },
                    );
                    outcomes[index] = Outcome::Conflict(conflict_index);
                    continue;
                }
                None => None,
            }
        } else {
            None
        };

        match candidate {
            Some(contact) if contact.source != ContactSource::Import => {
                let conflict_index = push_conflict(
                    &mut conflicts,
                    Conflict {
                        rows: vec![row.row],
                        kind: ConflictKind::ManualContact,
                        candidates: vec![contact],
                    },
                );
                outcomes[index] = Outcome::Conflict(conflict_index);
            }
            Some(contact) => {
                outcomes[index] = Outcome::Action(action_for(row, &contact));
            }
            None => outcomes[index] = Outcome::Action(PlanAction::Add),
        }
    }
    progress.report(ImportPhase::Matching, total, total);

    // 4. One person cannot be targeted by two different rows.
    let mut by_contact: HashMap<i64, Vec<usize>> = HashMap::new();
    for (index, outcome) in outcomes.iter().enumerate() {
        if let Outcome::Action(action) = outcome
            && let Some(contact_id) = action_target(action)
        {
            by_contact.entry(contact_id).or_default().push(index);
        }
    }
    for (contact_id, indexes) in by_contact {
        if indexes.len() < 2 {
            continue;
        }
        let Some(contact) = by_id.get(&contact_id) else {
            continue;
        };
        let conflict_index = push_conflict(
            &mut conflicts,
            Conflict {
                rows: indexes.iter().map(|index| rows[*index].row).collect(),
                kind: ConflictKind::ContactMatchedTwice,
                candidates: vec![(*contact).clone()],
            },
        );
        for index in indexes {
            outcomes[index] = Outcome::Conflict(conflict_index);
        }
    }

    let mut planned_rows = Vec::with_capacity(rows.len());
    for (index, row) in rows.iter().enumerate() {
        let action = match &outcomes[index] {
            Outcome::Action(action) => action.clone(),
            Outcome::Conflict(conflict_index) => PlanAction::Conflict {
                index: *conflict_index,
            },
        };
        planned_rows.push(PlannedRow {
            input: row.clone(),
            action,
        });
    }

    // 6. Removals only for rosters, and only for people no conflict touches.
    let mut matched_ids: HashSet<i64> = HashSet::new();
    for row in &planned_rows {
        if let Some(contact_id) = action_target(&row.action) {
            matched_ids.insert(contact_id);
        }
    }
    let candidate_ids: HashSet<i64> = conflicts
        .iter()
        .flat_map(|conflict| conflict.candidates.iter().map(|candidate| candidate.id))
        .collect();
    let protected_keys: HashSet<&str> = errors
        .iter()
        .filter_map(|error| error.identity_key.as_deref())
        .collect();

    let mut removals = Vec::new();
    let active_import_contacts = snapshot
        .contacts
        .iter()
        .filter(|contact| {
            contact.status == ContactStatus::Active && contact.source == ContactSource::Import
        })
        .count() as u64;
    if scope == ImportScope::Roster {
        for contact in &snapshot.contacts {
            if contact.status != ContactStatus::Active || contact.source != ContactSource::Import {
                continue;
            }
            if matched_ids.contains(&contact.id) || candidate_ids.contains(&contact.id) {
                continue;
            }
            if contact
                .identity_key
                .as_deref()
                .is_some_and(|key| protected_keys.contains(key))
            {
                continue;
            }
            removals.push(PlannedRemoval {
                contact: contact.clone(),
            });
        }
    }

    let mut warnings = Vec::new();
    let removal_count = removals.len() as u64;
    if scope == ImportScope::Roster
        && removal_count > 5
        && removal_count.saturating_mul(2) > active_import_contacts
    {
        warnings.push(PlanWarning::LargeRemoval {
            count: removal_count,
            active_import_contacts,
        });
    }

    let summary = summarize(rows, errors, &planned_rows, &removals, conflicts.len());
    Ok(ImportPlan {
        version: PLAN_VERSION,
        scope,
        identity_strategy: strategy_id.to_owned(),
        mapping: mapping.clone(),
        rows: planned_rows,
        removals,
        conflicts,
        errors: errors.to_vec(),
        warnings,
        summary,
    })
}

/// What applying `row` to `contact` would change.
pub fn action_for(row: &NormalizedRow, contact: &ContactRef) -> PlanAction {
    if contact.status == ContactStatus::Removed {
        return PlanAction::Restore {
            target: contact.clone(),
        };
    }
    if row.email.normalized != contact.email_normalized {
        return PlanAction::UpdateEmail {
            target: contact.clone(),
        };
    }
    if clean_name(&row.first_name) != clean_name(&contact.first_name)
        || clean_name(&row.last_name) != clean_name(&contact.last_name)
    {
        return PlanAction::UpdateDetails {
            target: contact.clone(),
        };
    }
    PlanAction::Noop {
        contact_id: contact.id,
    }
}

/// Turn conflict answers into ordered operations.
///
/// Operations follow `plan.rows`; removals come last. No contact may be modified
/// twice and each new address may be created once.
pub fn resolve(
    plan: &ImportPlan,
    resolutions: &[ConflictResolution],
) -> CoreResult<Vec<ContactOp>> {
    let conflict_rows: HashSet<u32> = plan
        .conflicts
        .iter()
        .flat_map(|conflict| conflict.rows.iter().copied())
        .collect();
    let mut by_row: HashMap<u32, &ConflictResolution> = HashMap::new();
    for resolution in resolutions {
        if !conflict_rows.contains(&resolution.row) {
            return Err(CoreError::Validation {
                field: "resolutions",
                reason: format!("Row {} is not waiting for a decision.", resolution.row),
            });
        }
        if by_row.insert(resolution.row, resolution).is_some() {
            return Err(CoreError::Validation {
                field: "resolutions",
                reason: format!("Row {} has more than one decision.", resolution.row),
            });
        }
    }
    let missing = conflict_rows
        .iter()
        .filter(|row| !by_row.contains_key(row))
        .count();
    if missing > 0 {
        return Err(CoreError::InvalidState(format!(
            "{missing} rows need a decision"
        )));
    }

    let conflict_of_row: HashMap<u32, &Conflict> = plan
        .conflicts
        .iter()
        .flat_map(|conflict| conflict.rows.iter().map(move |row| (*row, conflict)))
        .collect();

    let mut ops = Vec::new();
    let mut targeted: HashSet<i64> = HashSet::new();
    let mut created_emails: HashSet<&str> = HashSet::new();

    for planned in &plan.rows {
        let row = &planned.input;
        let action = match &planned.action {
            PlanAction::Conflict { .. } => {
                let Some(resolution) = by_row.get(&row.row) else {
                    continue;
                };
                let Some(conflict) = conflict_of_row.get(&row.row) else {
                    continue;
                };
                match &resolution.action {
                    ConflictAction::Skip => continue,
                    ConflictAction::CreateNew => {
                        if conflict
                            .candidates
                            .iter()
                            .any(|candidate| candidate.email_normalized == row.email.normalized)
                        {
                            return Err(CoreError::Validation {
                                field: "resolutions",
                                reason: format!(
                                    "Row {}: a live contact already has this email; link it instead.",
                                    row.row
                                ),
                            });
                        }
                        if !created_emails.insert(row.email.normalized.as_str()) {
                            return Err(CoreError::Validation {
                                field: "resolutions",
                                reason: format!(
                                    "Row {}: this email is already being created by another row.",
                                    row.row
                                ),
                            });
                        }
                        ops.push(ContactOp::Create {
                            row: row.row,
                            first_name: row.first_name.clone(),
                            last_name: row.last_name.clone(),
                            email: row.email.clone(),
                            identity_key: row.identity_key.clone(),
                        });
                        continue;
                    }
                    ConflictAction::LinkContact { contact_id } => {
                        let Some(candidate) = conflict
                            .candidates
                            .iter()
                            .find(|candidate| candidate.id == *contact_id)
                        else {
                            return Err(CoreError::Validation {
                                field: "resolutions",
                                reason: format!(
                                    "Row {}: contact {contact_id} is not one of the choices.",
                                    row.row
                                ),
                            });
                        };
                        action_for(row, candidate)
                    }
                }
            }
            action => action.clone(),
        };

        match &action {
            PlanAction::Add => ops.push(ContactOp::Create {
                row: row.row,
                first_name: row.first_name.clone(),
                last_name: row.last_name.clone(),
                email: row.email.clone(),
                identity_key: row.identity_key.clone(),
            }),
            PlanAction::UpdateDetails { target }
            | PlanAction::UpdateEmail { target }
            | PlanAction::Restore { target } => {
                if !targeted.insert(target.id) {
                    return Err(CoreError::InvalidState(format!(
                        "Contact {} would be changed twice; resolve the conflicts differently.",
                        target.id
                    )));
                }
                ops.push(modify_op(row, &action, target)?);
            }
            PlanAction::Noop { .. } | PlanAction::Conflict { .. } => {}
        }
    }

    for removal in &plan.removals {
        if !targeted.insert(removal.contact.id) {
            return Err(CoreError::InvalidState(format!(
                "Contact {} would be changed twice; resolve the conflicts differently.",
                removal.contact.id
            )));
        }
        ops.push(ContactOp::Modify {
            row: None,
            contact_id: removal.contact.id,
            expected_version: removal.contact.version,
            first_name: None,
            last_name: None,
            email: None,
            status: Some(ContactStatus::Removed),
            identity_key: None,
        });
    }

    Ok(ops)
}

fn modify_op(
    row: &NormalizedRow,
    action: &PlanAction,
    target: &ContactRef,
) -> CoreResult<ContactOp> {
    let restore = matches!(action, PlanAction::Restore { .. });
    let email_differs = row.email.normalized != target.email_normalized;
    let first_differs = clean_name(&row.first_name) != clean_name(&target.first_name);
    let last_differs = clean_name(&row.last_name) != clean_name(&target.last_name);
    if !restore && !email_differs && !first_differs && !last_differs {
        return Err(CoreError::InvalidState(
            "The plan contains a change with nothing to change.".to_owned(),
        ));
    }
    Ok(ContactOp::Modify {
        row: Some(row.row),
        contact_id: target.id,
        expected_version: target.version,
        first_name: first_differs.then(|| row.first_name.clone()),
        last_name: last_differs.then(|| row.last_name.clone()),
        email: email_differs.then(|| row.email.clone()),
        status: restore.then_some(ContactStatus::Active),
        identity_key: (first_differs || last_differs)
            .then(|| row.identity_key.clone())
            .flatten(),
    })
}

fn summarize(
    rows: &[NormalizedRow],
    errors: &[RowError],
    planned_rows: &[PlannedRow],
    removals: &[PlannedRemoval],
    conflicts: usize,
) -> ImportSummary {
    let mut summary = ImportSummary {
        row_count: (rows.len() + errors.len()) as u64,
        invalid: errors.len() as u64,
        conflicts: conflicts as u64,
        removed: removals.len() as u64,
        ..ImportSummary::default()
    };
    for row in planned_rows {
        match row.action {
            PlanAction::Add => summary.added += 1,
            PlanAction::UpdateDetails { .. }
            | PlanAction::UpdateEmail { .. }
            | PlanAction::Restore { .. } => summary.updated += 1,
            PlanAction::Noop { .. } => summary.unchanged += 1,
            PlanAction::Conflict { .. } => {}
        }
    }
    summary
}

fn push_conflict(conflicts: &mut Vec<Conflict>, conflict: Conflict) -> usize {
    conflicts.push(conflict);
    conflicts.len() - 1
}

fn action_target(action: &PlanAction) -> Option<i64> {
    match action {
        PlanAction::UpdateDetails { target }
        | PlanAction::UpdateEmail { target }
        | PlanAction::Restore { target } => Some(target.id),
        PlanAction::Noop { contact_id } => Some(*contact_id),
        PlanAction::Add | PlanAction::Conflict { .. } => None,
    }
}

fn index_by<'a, F>(contacts: &[&'a ContactRef], key: F) -> HashMap<String, Vec<&'a ContactRef>>
where
    F: Fn(&ContactRef) -> String,
{
    let mut map: HashMap<String, Vec<&ContactRef>> = HashMap::new();
    for contact in contacts {
        let key = key(contact);
        if key.is_empty() {
            continue;
        }
        map.entry(key).or_default().push(*contact);
    }
    map
}

fn history_index<'a>(
    snapshot: &'a ReconcileSnapshot,
    by_id: &HashMap<i64, &'a ContactRef>,
) -> HashMap<&'a str, Vec<i64>> {
    let mut map: HashMap<&str, Vec<i64>> = HashMap::new();
    for (email, contact_id) in &snapshot.history {
        if by_id.contains_key(contact_id) {
            map.entry(email.as_str()).or_default().push(*contact_id);
        }
    }
    map
}

fn distinct(ids: &[i64]) -> usize {
    ids.iter().copied().collect::<HashSet<_>>().len()
}

fn distinct_refs(ids: &[i64], by_id: &HashMap<i64, &ContactRef>) -> Vec<ContactRef> {
    let mut seen = HashSet::new();
    ids.iter()
        .filter(|id| seen.insert(**id))
        .filter_map(|id| by_id.get(id))
        .map(|contact| (*contact).clone())
        .collect()
}
