# 09 — Parity and readiness checklist

A living checklist used to decide when the rewrite can replace the TUI. Each item needs
evidence: a passing test, a recorded manual check, or a linked artifact. Do not check an
item because the code exists.

Status: **phase 1 implemented** (see [02-exec.md](02-exec.md)); phases 2–7 not started.
Phase numbers refer to [06-delivery-plan.md](06-delivery-plan.md).

> **Premise change (2026-11-11):** no legacy data support — see
> [ADR 0006](../adr/0006-clean-rewrite-no-legacy-data.md). Items about importing or
> discovering the TUI database are void; parity means equivalent workflows, not
> migrated data.

## Foundation (phase 1)

- [x] `desktop/` SvelteKit app builds with `adapter-static` and runs in a Tauri window.
      Evidence: static adapter in [`desktop/vite.config.ts`](../../desktop/vite.config.ts), shell
      wired in [`desktop/src-tauri/src/lib.rs`](../../desktop/src-tauri/src/lib.rs); a debug window
      reached the dashboard against a real SQLite file on 2026-11-11; Playwright shell test
      [`desktop/tests/e2e/shell.spec.ts`](../../desktop/tests/e2e/shell.spec.ts).
- [x] Rust workspace builds with format/clippy/test clean.
      Evidence: `pnpm verify` locally (fmt, clippy `-D warnings`, `cargo test --workspace`, layer
      check) on 2026-11-11; CI job `rust` added (not yet run remotely).
- [x] Generated IPC bindings are committed and CI checks they are current.
      Evidence: [`desktop/src/lib/ipc/bindings.ts`](../../desktop/src/lib/ipc/bindings.ts) is
      generated and committed; [`desktop/src-tauri/tests/ipc_contract.rs`](../../desktop/src-tauri/tests/ipc_contract.rs)
      compares it with every registered descriptor; CI job `contract` runs
      `pnpm bindings && git diff --exit-code`.
- [x] Feature registry renders navigation from manifests (no hard-coded nav).
      Evidence: [`desktop/src/lib/features/registry.ts`](../../desktop/src/lib/features/registry.ts),
      [`desktop/src/lib/features/registry.test.ts`](../../desktop/src/lib/features/registry.test.ts),
      [`desktop/src/lib/shell/AppShell.svelte`](../../desktop/src/lib/shell/AppShell.svelte) renders
      `registry.current`; the boundary test forbids cross-feature imports.
- [x] `pnpm gen feature demo --nav` produces a working module without shell edits.
      Evidence: [`tools/gen-feature.test.mjs`](../../tools/gen-feature.test.mjs) and a real run on
      2026-11-11 that added `/demo` to the sidebar and palette; the generated command returned an
      empty list from the Rust backend, then the slice was reverted. No shell or registry edits.
- [x] Capabilities are least-privilege; CSP blocks remote content.
      Evidence: [`desktop/src-tauri/capabilities/default.json`](../../desktop/src-tauri/capabilities/default.json)
      grants only `core:default` and `log:default`; the CSP in
      [`desktop/src-tauri/tauri.conf.json`](../../desktop/src-tauri/tauri.conf.json) blocked an
      in-app `fetch("https://example.com")` (manual probe, no violation event exposed, request failed).

## Data safety (phase 2)

- [ ] All migrations run from zero on a fresh database.
- [ ] A pre-migration backup is created and restore is exercised.
- [ ] `PRAGMA foreign_keys=ON` and `PRAGMA foreign_key_check` is clean after every write.
- [ ] Legacy import matches contact/campaign/batch/record counts.
- [ ] Re-running legacy import does not duplicate data.
- [ ] A forced mid-transaction failure leaves no partial import.
- [ ] Email changes append history; send records keep snapshots.

## Read parity (phase 3)

- [ ] Dashboard shows contact/campaign/send counts and recent activity.
- [ ] Freshies lists all contacts with name, email, status, source, batch, date added.
- [ ] Freshies search, sort and filters work server-side and survive reload via URL.
- [ ] Contact detail shows email history and campaigns received.
- [ ] Campaigns list/detail shows sent, unsent and total audience.
- [ ] Campaign audience tabs paginate and search.
- [ ] Send history lists batches newest-first and searches recipients/notes.
- [ ] Batch detail shows snapshotted recipients.
- [ ] Imports list shows source, status and counts; batch detail lists members.
- [ ] Global command-palette search finds contacts, campaigns and batches.

## Write parity (phase 4)

- [ ] Create/edit/archive campaign with validation and version conflict handling.
- [ ] Prepare send previews recipients and disables when none are unsent.
- [ ] Prepare send writes exactly the previewed addresses to the clipboard.
- [ ] Prepare send creates one batch and one immutable record per recipient.
- [ ] Clipboard failure creates no batch.
- [ ] Re-copying a batch returns the same snapshotted addresses.
- [ ] Import wizard maps columns and previews add/update/remove/no-op counts.
- [ ] Applying an import is atomic and idempotent for an unchanged sheet.
- [ ] Discarding a draft leaves contacts untouched.
- [ ] Manual contact create/edit works and never appears in roster removals.
- [ ] Mark removed / restore / archive work with confirmation.
- [ ] Every mutation writes an audit event.
- [ ] Backend events invalidate the right queries without full reloads.

## Reconciliation (phase 5)

- [ ] Changed email updates the same contact and records history.
- [ ] Roster removal marks contacts removed (never deletes).
- [ ] Reappearing contact is restored.
- [ ] Duplicate email rows are rejected with a clear conflict.
- [ ] Ambiguous identity produces a conflict instead of a guess.
- [ ] Conflict queue supports link/create/skip and blocks apply while unresolved.
- [ ] Delta imports never remove anyone.
- [ ] Import undo restores the pre-import state and is audited.

## Quality and scale (phase 6)

- [ ] Contacts list p95 < 100 ms at 50k rows on the target machine.
- [ ] 10k-row import keeps the UI responsive and shows progress.
- [ ] All primary flows are keyboard operable; shortcuts are documented in-app.
- [ ] Primary routes pass automated accessibility checks.
- [ ] Light/dark themes meet contrast checks.
- [ ] Destructive actions require confirmation; history has no delete action.
- [ ] Diagnostics exposes versions, schema, DB path, logs, backup and open-folder.
- [ ] Errors provide a next step and never fail silently.

## Release (phase 7)

- [ ] Installers build on macOS, Windows and Linux CI.
- [ ] macOS bundle is signed/notarized; Windows bundle is signed.
- [ ] Updater manifest is signed and a staging update applies successfully.
- [ ] Upgrade preserves database and settings.
- [ ] TUI database discovery/import is documented and tested.
- [ ] Legacy app remains runnable until 1.0 sign-off.
- [ ] Rollback procedure is documented and rehearsed.

## Extensibility proof (ongoing)

- [ ] At least two features were added after 1.0 using the feature recipe without
      editing the shell or unrelated feature code.
- [ ] A new import format was added by implementing `ContactImporter` only.
- [ ] A new schema field arrived through an additive migration with no data rewrite.
- [ ] A feature flag disables a module without breaking navigation or events.

## TUI decommission gate

The TUI can be removed from the active workflow only when:

- every item in phases 1–5 is checked;
- a real user has completed a full import → campaign → send → re-copy cycle in the GUI;
- the legacy database migration has been run on a copy of the production database and
  the validation report is archived;
- a restore from backup has been demonstrated.
