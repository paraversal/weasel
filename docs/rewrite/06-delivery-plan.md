# 06 — Delivery plan

Phases are ordered so the extensibility mechanism is proven before most feature code is
written, and so data safety lands before any write path. Estimates are ideal engineering
days for one developer and exclude review lag.

**Critical path:** 0 → 1 → 2 → 3 → 4 → 5 → 6 → 7.

> **Premise change (2026-11-11):** no legacy data support — see
> [ADR 0006](../adr/0006-clean-rewrite-no-legacy-data.md). Affects this plan:
> phase 0 keeps the Python app as a reference only (not runnable-until-parity);
> phase 2 ships the data core without the legacy import; phase 7 has no legacy
> migration or removal step.

| Phase | Outcome | Estimate |
| --- | --- | --- |
| 0 | Legacy app preserved, decisions recorded | 0.5–1 d |
| 1 | Runable Tauri + SvelteKit shell with typed IPC and feature registry | 3–5 d |
| 2 | Target schema, repositories, legacy import, tests | 4–6 d |
| 3 | Read-only parity: all current screens | 4–6 d |
| 4 | Write parity: campaigns, sends, imports, manual contacts | 5–7 d |
| 5 | Roadmap features: reconciliation, removals, conflicts, manual exclusions | 4–6 d |
| 6 | Scale, polish, accessibility, diagnostics | 4–6 d |
| 7 | Signed installers, updater, migration guide | 3–5 d |
| 8 | Post-parity features, one module at a time | ongoing |

Recommended release line:

- **Internal alpha** at the end of phase 4 (feature parity for the happy path).
- **Beta** at the end of phase 5 (roster reconciliation complete; data-safe).
- **1.0** at the end of phase 7.

## Phase 0 — Preserve and decide (0.5–1 d)

Tasks:

- Tag the current commit (`tui-final`) before structural changes.
- Move `main.py`, `src/`, `pyproject.toml`, `uv.lock` and Python notebooks into
  `legacy/python-tui/`; keep the TUI runnable from there with a one-line path/import fix.
- Record ADRs: Tauri/SvelteKit, Rust-only backend, SQLite retention, `Contact` naming,
  `pnpm`, static SPA.
- Freeze the feature-parity checklist from [01-current-state.md](01-current-state.md).

Acceptance:

- `git tag tui-final` exists.
- The legacy TUI starts from its new home against a copy of a legacy database.
- No rewrite code exists in the root `src/`; the new `desktop/` tree is separate.

## Phase 1 — Foundation and the extension seam (3–5 d)

Tasks:

- Scaffold `desktop/` with SvelteKit + TypeScript strict + Tailwind + adapter-static,
  `ssr = false`, SPA fallback.
- Scaffold `desktop/src-tauri/` with Tauri 2, plugins (dialog, clipboard, opener, log,
  single-instance, updater later), capabilities, CSP.
- Create the Rust workspace and empty `weasel-core`, `weasel-store`, `weasel-import`,
  `weasel-platform`, `weasel-testkit` crates with the dependency rule enforced by review
  (and optionally `cargo-deny`/`deny.toml`).
- Implement `AppError`, `AppState`, logging, `app_info`, `app_health`, `settings_get_all`.
- Implement `specta`/`tauri-specta` export as `pnpm bindings`; commit generated bindings.
- Build the app shell: sidebar, topbar, command palette, toasts, theme, error boundary.
- Implement the feature manifest types, registry, and `tools/gen-feature.mjs`.
- Add dashboard feature and a hidden "developer" page that lists registered feature
  descriptors (proves discovery).
- Add CI skeleton: frontend check/lint/test/build, Rust fmt/clippy/test, bindings diff.

Acceptance:

- `pnpm tauri dev` opens a window on macOS; the same commands are documented for
  Windows/Linux.
- `pnpm bindings && git diff --exit-code` passes.
- `pnpm gen feature demo --nav` creates a compiling frontend module and backend command
  stub; the nav entry appears without editing `AppShell.svelte`.
- CI is green on a clean checkout.
- No business logic exists in Svelte components; the demo feature proves the command
  round trip.

## Phase 2 — Data core and legacy migration (4–6 d)

Tasks:

- Write `0001_initial.sql` from [03-data-model.md](03-data-model.md).
- Implement `SqliteStore` pool setup: app-data path, WAL, `foreign_keys=ON`,
  `busy_timeout`, migrations and pre-migration backup.
- Implement repositories and application services for contacts, campaigns, sends,
  imports, settings and audit. No UI yet.
- Implement `legacy_import.rs` with count/hash validation and a dry-run report.
- Build `weasel-testkit`: temp database, migration runner, deterministic clock,
  fixtures.
- Add a developer command/CLI to create a demo database.

Acceptance:

- `cargo test --workspace` covers repositories and services.
- A real legacy DB imports with matching counts; `PRAGMA foreign_key_check` is empty.
- Re-running the import refuses to duplicate data.
- Every write runs in a transaction; a forced failure rolls back completely.
- A backup file exists before migration and is restorable.

## Phase 3 — Read parity (4–6 d)

Tasks:

- Contacts screen: server-paginated table, search, status/batch filters, sort, row detail
  with email history and campaign participation.
- Dashboard: contact counts, active/removed, recent imports, recent sends.
- Campaigns: list and detail, sent/unsent/all audience tabs, stats.
- Send history: batch list, search, batch detail with recipient snapshots.
- Imports: batch list, summary, status, per-batch member list.
- Global search command for the palette.
- Query keys, loading/empty/error states, URL-synced filters.

Acceptance:

- All TUI read workflows are available without the TUI.
- Tables paginate; no screen loads the full contacts table.
- Search and filters survive reload via URL state.
- An import and a send performed via the legacy app appear correctly after migration.

## Phase 4 — Write parity (5–7 d)

Tasks:

- Create/edit/archive campaign with validation and optimistic concurrency.
- Prepare send: confirmation, recipient preview, clipboard write, atomic batch + record
  creation, success toast with "view batch".
- Re-copy an existing batch; edit batch note.
- Import wizard: pick file → detect headers → column mapping → roster/delta scope →
  preview diff → apply/discard; progress events; per-row outcomes.
- Manual contact create/edit; mark removed/restore; hard delete prohibited when history
  exists.
- Audit events for every mutation; activity timeline on contact/campaign detail.
- Event emission and targeted query invalidation.

Acceptance:

- A full happy-path session (import a sheet, create a campaign, copy recipients, re-copy
  from history) works end to end.
- Clipboard contains exactly the previewed recipient set.
- If the clipboard write fails, no send batch is recorded.
- Re-importing the same sheet produces zero additions and zero removals (idempotent).
- Editing a contact email does not change any past send record.
- Concurrent edit from two windows yields a `CONFLICT`, not silent overwrite.

## Phase 5 — Roster reconciliation and conflicts (4–6 d)

Tasks:

- `IdentityStrategy` implementation with configurable columns.
- Matching through current email, email history, identity key.
- Roster removals (mark removed, never delete), restore flow.
- Conflict queue UI: link to existing contact, create new, skip; bulk resolutions.
- Delta vs roster scope and manual-contact exclusion.
- Reconciliation test matrix and fixtures.
- "Undo this import" via the audit trail (compensating actions, not row deletion).

Acceptance (each is an automated integration test plus one UI test):

- Changed email updates the same contact and appends history.
- Person removed from the roster becomes `removed`, not deleted; history remains.
- Removed person reappearing is restored.
- Same email in two source rows is rejected with a clear conflict.
- Duplicate names with different emails create a conflict instead of a guess.
- Manual contacts never appear in `mark_removed` proposals.
- Applying an import with unresolved conflicts is rejected.
- Undo restores the pre-import state and records its own audit event.

## Phase 6 — Scale, polish, accessibility (4–6 d)

Tasks:

- Virtualized recipient/contact tables; measure at 50k contacts.
- Add FTS5 only if `LIKE` misses the performance budget; keep the command contract.
- Keyboard shortcuts and command palette coverage; focus management.
- Empty/loading/skeleton states; helpful first-run screen; in-app help.
- Theme (system/light/dark), density, column visibility persistence.
- Diagnostics screen: versions, schema, DB path, log location, backup, open data folder.
- Accessibility pass: labels, contrast, focus trap, reduced motion, screen-reader test.
- Crash/error reporting to local logs; no silent failures.

Acceptance:

- p95 contacts list query < 100 ms at 50k rows on the target machine.
- All primary flows are reachable and operable with the keyboard only.
- Automated axe checks have no serious violations on main screens.
- Import of 10k rows keeps the UI responsive and shows progress.

## Phase 7 — Packaging and release (3–5 d)

Tasks:

- App icons, bundle identifiers, metadata, license and versioning.
- macOS signing/notarization, Windows signing, Linux AppImage/deb.
- Updater with signed manifests and a beta channel.
- GitHub Actions matrix builds using `tauri-action`.
- Migration guide for TUI users (where the DB lives, automatic copy, backup).
- Release checklist and rollback procedure.

Acceptance:

- Fresh installs run on macOS, Windows and Linux CI runners.
- Upgrade from the previous app version preserves the database and settings.
- A TUI database is discovered/copied on first run or imported explicitly.
- An update can be published and applied in a staging channel.

## Phase 8 — Post-parity features (ongoing)

Each is a separate feature module and can ship independently:

1. Campaign templates and variable substitution (`{{first_name}}`), preview, validation.
2. Tags, saved segments, and audience rules for campaign targeting.
3. Direct sending through an `EmailProvider` (SMTP/API), per-recipient delivery status.
4. Scheduling and reminders.
5. Import from CSV/clipboard/Google Sheets with the same reconcile pipeline.
6. Export contacts and send history (CSV/xlsx) through an `Exporter` port.
7. Custom fields and per-contact metadata UI.
8. Activity timeline and richer audit/undo.
9. Multi-window campaign composer.
10. Optional WASM extension host for third-party importers/filters.

## Dependencies and risks

| Risk | Mitigation |
| --- | --- |
| Spreadsheet edge cases (merged headers, totals rows, locale) | Explicit column mapping, preview, fixture corpus from real sheets, clear validation errors |
| Legacy DB anomalies (orphan send rows, mixed campaigns per paste) | Dry-run report, split/repair strategy, refuse-and-report rather than guess; backup first |
| Tauri IPC drift | Generated bindings + CI diff check |
| Feature registry becomes a hidden god object | Manifests only describe metadata/hooks; business logic stays in features/services |
| SQLite write contention | Single writer, WAL, short transactions, no long import transaction during parsing; apply is one transaction |
| Large imports freeze UI | `spawn_blocking`, progress events, streaming rows, 100-row previews |
| Clipboard failure vs DB commit | Clipboard before commit; rollback on failure; idempotent retry |
| macOS permissions (files, clipboard) | Native dialogs, explicit error messages, test on clean machine |
| Renaming Freshie → Contact confuses users | Keep "Freshies" in UI copy; use `Contact` only in code and docs |
| Rewrite stalls before parity | Feature-parity checklist, alpha cut at phase 4, legacy app remains runnable |

## Definition of done for 1.0

- Feature parity checklist complete and demonstrated.
- Legacy migration verified on at least three real databases (or anonymized copies).
- Every write is transactional, audited and covered by an integration test.
- No known data-loss bug; backups and restore exercised.
- CI green on all three platforms; installers signed/notarized where applicable.
- Extension recipe actually used at least twice (for example, a demo feature and one
  post-parity feature) without edits to the shell.
- User-facing documentation and in-app help match the shipped UI.
