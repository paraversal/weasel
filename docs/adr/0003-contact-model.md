# ADR 0003 — Contact model with soft removal and immutable send history

- **Status:** Accepted
- **Date:** 2026-11-11

## Context

The TUI identifies people only by email, silently ignores duplicates, cannot represent
roster removals, and derives past send history by joining the live person row. The
roadmap explicitly calls for handling changed emails and people leaving the programme.

## Decision

Model a person as `Contact` (UI copy remains "Freshies"), with:
- a current email plus normalized form and an email-history table;
- `active` / `removed` / `archived` status instead of hard deletion;
- an `identity_key` for matching when email changes;
- send records that snapshot name and email, linking to a contact only as a nullable
  reference;
- import batches with a roster/delta scope, a persisted preview plan and an audit trail.

## Consequences

- Email changes and roster removals become first-class, reviewable operations.
- Past send history remains truthful after edits or removals.
- Schema is larger and import reconciliation is more complex than `INSERT OR IGNORE`.
- Hard deletion is not a normal action; users archive instead. A future privacy-erasure
  flow, if required, must be designed explicitly.
- The UI keeps familiar "freshie" language while the code uses a reusable `Contact`
  concept for future audiences.

## Alternatives considered

- **Keep email as the primary identity:** cannot handle the roadmap's changed-email case
  without duplicates.
- **Hard-delete removed people:** destroys history and makes accidental removal
  unrecoverable.
- **Mutable send rows joined to contacts:** history rewrites itself when a contact
  changes.
