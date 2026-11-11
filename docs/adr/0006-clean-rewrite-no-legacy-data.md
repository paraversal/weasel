# ADR 0006 — Clean rewrite; the TUI database is not supported

- **Status:** Accepted
- **Date:** 2026-11-11
- **Deciders:** project owner, implementing agent

## Context

The rewrite plans (01–09) assumed the TUI's SQLite data would be migrated once into the
new schema: a `legacy_import` module, an `app_import_legacy_db` command, database
discovery on first run, a `legacy` import-batch kind, and keeping the TUI runnable
until parity was signed off.

The owner changed that premise on 2026-11-11:

> The current codebase is just a reference implementation. Its data model and older
> databases do not have to be supported. Treat this as a complete rewrite.

What happens if we do nothing: later exec plans continue to build import tooling,
fixtures and failure handling for a database format nobody needs to keep, and phase 1
carries phase-5 work.

## Decision

Treat the rewrite as a clean application.

- No legacy database import, no discovery flow, no `legacy` import-batch kind.
- No `legacy_import.rs`, no legacy dry-run report, no "keep the TUI runnable until
  parity" gate.
- The Python TUI is kept under `legacy/python-tui/` as a **read-only behavioural
  reference** (feature inventory, edge cases). Nothing imports it.
- Behaviour — workflows, not data — is the contract; `09-parity-checklist.md` still
  governs whether the rewrite may replace the TUI.
- The new app's **own** database still gets a pre-migration `VACUUM INTO` backup, so
  schema upgrades of our data stay safe.

## Consequences

| Planning-doc item | Status now |
| --- | --- |
| `weasel-store/src/legacy_import.rs`, legacy dry-run report (03, 07) | Dropped |
| `app_import_legacy_db` command (04) | Dropped |
| Legacy DB discovery on first run (07, 09 phase 7) | Dropped |
| `import_batches.source_kind = 'legacy'` (03) | Drop the value when 03 is executed |
| "Keep legacy TUI runnable until parity" (README, 06 phase 0, 09) | Dropped; Python code is reference only |
| Pre-migration backup of the app's own DB (02, 07) | Kept |
| Feature parity with TUI workflows (01, 09) | Kept |

Easier: phase 1 and 2 shrink; no fixtures needed from real rosters; no data-mapping
bugs to chase. Harder: existing users must re-import their rosters through the new
import flow; there is no recovery path from an old database. Accepted, because the
application has no deployed user base whose data must survive.

## Related decisions made while executing 02-exec

The following small deviations from the planning docs are recorded here so they have a
durable home. Details and rationale: `docs/rewrite/02-exec.md` §9.

1. `0001_app.sql` contains only `settings` + `app_meta`; domain tables arrive as
   `0002_domain.sql` in 03-exec.
2. Events use a `WeaselEvent { revision, event }` envelope instead of a `revision`
   field on every variant.
3. The event channel name is derived by `tauri_specta::Event`, not written by hand.
4. Capabilities are `core:default` + `log:default`; the backend performs dialog,
   clipboard and opener actions through ports.
5. `FeatureDescriptor` has no `migrations` field; migrations are one global ordered
   sequence.
6. The feature generator prints a migration reminder instead of scaffolding one.
7. One root `pnpm-lock.yaml` (pnpm workspace), not `desktop/pnpm-lock.yaml`.
8. Data directory override is `WEASEL_DATA_DIR` now; a config-file override is
   deferred to phase 6.
9. TanStack Query uses the v6 thunk form.
10. `settings` is a normal registry feature (no filter-out special case).
11. Tauri-backed adapters live in `desktop/src-tauri/src/adapters.rs`;
    `weasel-platform` stays Tauri-free.

## Related decisions made while executing 03-exec

12. `identity_key` is a plain normalized string (`lower(first)\x1flast`), not a hash;
    the fixed strategy id `name-v1` is recorded in `app_meta`. Configuration and
    recomputation are deferred to phase 5.
13. `ContactImporter` from 05 is not a core port. `SourceReader` lives in
    `weasel-import`, because parsing is the only consumer and core stays parsing-free.
14. The planner adds `AmbiguousHistory`, `ContactMatchedTwice`, empty-roster refusal,
    a large-removal warning and invalid-row protection beyond 03's matching rules.
15. Campaign audiences are active contacts only; removed people are not silently
    included in an unsent count (01 defect 2).

## Alternatives considered

- **One-shot legacy import** (the original plan): rejected — it costs a module, a
  command, fixtures and a UI flow for data that does not need to survive; the owner
  explicitly removed the need.
- **Keep the TUI runnable and maintained until parity**: rejected — it forces two
  implementations of every workflow and a second schema to keep honest, for a
  single-user internal tool.
- **Delete the Python code entirely** (tag `tui-final` preserves it): viable, but
  `01-current-state.md` points into the source, so keeping it costs almost nothing.
