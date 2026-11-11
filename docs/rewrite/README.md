# Weasel rewrite: Tauri 2 + SvelteKit

This directory is the plan for replacing the Textual TUI with a Tauri 2 + SvelteKit
desktop application. It is written to be executed in phases and to make adding new
features cheap, not just to reproduce the current screens.

**Status:** planning only. No application code has been moved or deleted.

## Documents

| Document | Purpose |
| --- | --- |
| [01-current-state.md](01-current-state.md) | Feature inventory, schema, and defects of the TUI that the rewrite must fix |
| [02-architecture.md](02-architecture.md) | Stack choice, repository layout, layering, runtime and request flow |
| [03-data-model.md](03-data-model.md) | Target SQLite schema, invariants, legacy-data mapping |
| [04-ipc-and-events.md](04-ipc-and-events.md) | Typed command contract, DTOs, errors, events, bindings generation |
| [05-extensibility.md](05-extensibility.md) | Feature-module system, extension points, and the recipe for adding a feature |
| [06-delivery-plan.md](06-delivery-plan.md) | Phased implementation plan with acceptance criteria |
| [07-testing-migration-release.md](07-testing-migration-release.md) | Test strategy, legacy-DB migration, CI and packaging |
| [08-ui-design.md](08-ui-design.md) | Screen map, interaction design, visual system and keyboard model |
| [09-parity-checklist.md](09-parity-checklist.md) | Living checklist and TUI decommission gate |

Decisions that are expensive to reverse are recorded as ADRs in [`../adr/`](../adr/).

## The short version

- **Shell:** Tauri 2 for the native window, filesystem, dialogs, clipboard, updater
  and packaging.
- **UI:** SvelteKit (Svelte 5, TypeScript, static SPA adapter), Tailwind CSS and
  shadcn-svelte-style components, TanStack Query for server state, TanStack Table for
  data grids.
- **Core:** a Rust workspace split into `weasel-core` (pure domain), `weasel-store`
  (SQLite repositories + migrations), `weasel-import` (file parsing and reconciliation)
  and `weasel-platform` (clipboard/dialog/filesystem ports). Tauri lives in a thin
  adapter crate; the domain never imports Tauri or SQLite.
- **Contract:** one typed IPC surface exported from Rust to TypeScript with
  `specta`/`tauri-specta`, so a backend change breaks the frontend build instead of
  failing at runtime.
- **Extensibility:** features are self-contained modules on both sides. Each declares
  its IPC commands, events, navigation entry, command-palette actions, settings section
  and migrations. A generator scaffolds a new module; a registry composes the shell.
- **Data:** SQLite with forward-only migrations, soft removals, import-history audit,
  and snapshotted send records so history survives email changes. The legacy database
  is imported once through a verified migration command.
- **Safety:** transactions around every write, a backup before schema migration, and no
  destructive defaults. The current "delete all campaigns" debug action does not carry
  over to the normal UI.

## Decisions already made by this plan

These are deliberate defaults so implementation does not stall. They can be changed,
but changing one should be recorded in `docs/adr/`.

1. Rust replaces Python for the application backend; no Python sidecar. `calamine`
   covers `.xlsx`/`.xls`/`.ods` and `csv` covers CSV. The old Python app is kept under
   `legacy/` until parity is signed off.
2. SQLite remains the store. No server component and no network dependency for core
   features.
3. The domain entity is `Contact`. The UI keeps the project's "Freshies" vocabulary for
   imported people; the generic name lets the same core serve non-freshie audiences
   later.
4. `pnpm` is the package manager and the frontend is a static SPA served from the Tauri
   bundle. There is no SSR at runtime.
5. New code has a hard dependency rule: `ui -> ipc -> application -> domain` and
   `adapter -> application`; domain depends on nothing framework-specific.

## Open questions (non-blocking for phase 1)

These do not block scaffolding; they should be answered before the phase that needs
them:

1. **Bundle identifier and product metadata** — the plan assumes
   `dev.paraversal.weasel`. Needed before signed builds (phase 7).
2. **Direct sending** — the plan keeps "copy addresses to the clipboard" as the send
   mechanism. If SMTP/provider sending is wanted soon, the `EmailProvider` port and a
   `sending` feature should be scheduled after phase 5.
3. **Identity columns** — changed-email matching starts with first + last name. If the
   real rosters contain a stable matriculation/student number, include it in the identity
   key during phase 5.
4. **Legacy database location** — the migration flow supports discovery and explicit
   file selection. Providing one anonymized real database before phase 2 would improve
   migration fixtures.

## Suggested first commit after this plan

1. Move the current implementation to `legacy/python-tui/` without edits (keep
   `git history` authoritative).
2. Scaffold `desktop/` (SvelteKit) and `desktop/src-tauri/` (Tauri 2), plus the Rust
   workspace and CI.
3. Land the typed IPC skeleton and the feature registry with a single `dashboard`
   feature before implementing any real screen.

That order proves the extension mechanism before a large amount of feature code is
written on top of it.
