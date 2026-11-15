# AGENTS.md

Operating manual for this repository. What exists and where it is going is in
[`HANDOFF.md`](HANDOFF.md); read that once, then come back here for the rules.

## Commands

| Command | Use it for |
| --- | --- |
| `pnpm verify` | **The gate.** `check && lint && test`, then `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `tools/check-layers.sh`. Run before saying a change is done. |
| `pnpm dev` | Tauri app in dev mode against your real data directory. |
| `pnpm dev:demo` | Same, but seeds a throwaway database and points `WEASEL_DATA_DIR` at it. Use this for any UI work. |
| `pnpm check` | `svelte-check` with `--fail-on-warnings`. |
| `pnpm lint` | Prettier `--check` plus ESLint. `pnpm -C desktop format` rewrites formatting. |
| `pnpm test` | `vitest run` in `desktop/`. |
| `pnpm bindings` | Regenerate `desktop/src/lib/ipc/bindings.ts` from the Rust command signatures. |
| `pnpm seed:demo -- --contacts 5000` | Seed a demo database (`--data-dir`, `--contacts`, `--seed`, `--force`). |
| `cargo test -p weasel-import` | Narrower than `pnpm verify` while iterating. |

CI (`.github/workflows/ci.yml`) runs the same checks in four jobs: frontend
(`check`, `lint`, `test`, `build`), rust (`fmt --check`, clippy `-D warnings`,
`cargo test --workspace`, layer check), contract (`pnpm bindings` then
`git diff --exit-code`), and a debug bundle build on macOS, Windows and Linux.

## Layout

```
crates/weasel-core/      pure domain: entities, validation, query/sort/filter, audit, errors, ports
crates/weasel-store/     SQLite: SQL, migrations, transactions, backup
crates/weasel-import/    file readers (csv, sheet), header detection, mapping, reconcile
crates/weasel-platform/  OS effects: clipboard, dialogs, opener, clock
crates/weasel-testkit/   seed + test doubles (TempDb, fake clock/clipboard)
desktop/src-tauri/       Tauri shell: commands, events, DTOs, error mapping, app state
desktop/src/             SvelteKit UI
migrations/              forward-only SQL, embedded at compile time
tools/check-layers.sh    dependency rules
```

Frontend (`desktop/src/lib/`):

```
routes/<x>/+page.svelte   thin: renders the screen component
screens/<feature>/        the actual screens and their local components
shell/                    layout, sidebar, palette, toasts, nav.ts, icons.ts
ipc/                      bindings.ts (generated), client.ts, queries.ts, events.ts, dragdrop.ts
stores/                   runes stores: term, theme, toasts, drop
components/app/           shared primitives (DataTable, FormError, EmptyState, …)
components/ui/            shadcn-svelte primitives; generated, keep edits minimal
utils/                    format, list-params, shortcuts, paths, errors, motion
```

## Dependency rules

`tools/check-layers.sh` fails the build if a crate grows one of these edges (shipped
dependencies only; dev-dependencies are exempt):

| Crate | Must not depend on |
| --- | --- |
| `weasel-core` | tauri, sqlx, calamine, specta, tokio, weasel-store, weasel-import, weasel-platform |
| `weasel-import` | tauri, sqlx, weasel-store, weasel-platform, tokio |
| `weasel-store` | tauri, specta, weasel-import, weasel-platform |
| `weasel-platform` | tauri, sqlx, weasel-store, weasel-import |

The spirit of it: `core` decides, `store` persists, `platform` touches the OS, `tauri`
wires them together, `desktop/src` renders. When a rule blocks you, move the logic to
where it belongs instead of adding an exception to the script.

Workspace lints (root `Cargo.toml`): `unsafe_code = "forbid"`, clippy `all = warn`,
`unwrap_used = warn`, `expect_used = warn`, `dbg_macro = warn`. `unwrap` in tests is
fine; in shipped code return the error.

## Backend rules

- **One transaction per command.** Use `store.begin_write()` for anything that writes;
  never leave a command half-applied.
- **Every mutation is audited.** Write to `audit_events` with an action from
  `weasel_core::audit::actions`, so the activity views stay honest.
- **Contacts are never deleted.** `active → removed → archived`; removal frees the email
  for reuse and keeps history.
- **Send batches are immutable.** Recipients, addresses and campaign text are snapshotted
  into `send_records` at prepare time; editing a contact later must not rewrite history.
- **Optimistic concurrency.** Contacts and campaigns carry `version`; an update with a
  stale version returns `CONFLICT` and the mutation is rejected.
- **Migrations are forward-only.** Add `migrations/000N_<topic>.sql` with the next
  number, never edit an applied file; `Store::open` takes a `VACUUM INTO` backup before
  applying. `crates/weasel-store/tests/schema.rs::schema_snapshot_is_stable` will need its
  expected snapshot updated.
- **Errors cross the IPC boundary as `AppError`.** `ErrorCode` is one of `VALIDATION`,
  `NOT_FOUND`, `CONFLICT`, `INVALID_STATE`, `IO`, `IMPORT`, `CANCELLED`, `INTERNAL`.
  Map domain errors with `From`; do not invent codes. `AppError::internal` logs a
  `log_id` and redacts emails — keep it that way for anything carrying addresses.
- **Do not block the UI thread.** Long work (imports, email checks) emits progress events
  and returns a handle; the frontend already has `importProgress` and `emailChecksChanged`.

## Frontend rules

- **Tauri only through `$lib/ipc`.** ESLint bans importing `@tauri-apps/api/core` and
  `@tauri-apps/api/event` anywhere else.
- **Toasts only through the store.** `svelte-sonner` is importable only in
  `$lib/stores/toasts.svelte.ts`, `lib/shell/Toasts.svelte` and the shadcn wrapper.
- **Navigation through `$lib/utils/paths`.** Links use `href(path)`; programmatic
  navigation uses `go(path)` (the single caller of `goto`). ESLint's
  `svelte/no-navigation-without-resolve` enforces it.
- **Query keys live in `queries.ts`.** Events invalidate them from `events.ts`, where the
  `satisfies Record<EventType, QueryKey[]>` map makes a new event variant a type error
  until it is listed. Never invalidate by hand after a mutation.
- **Report failures with `reportError(err)`** from `$lib/utils/errors` (a `CONFLICT`
  becomes "this changed elsewhere"), and render field errors with `fieldError(err,
  field)` next to the input. Backend validation messages are user-facing; do not
  duplicate them in the UI.
- **Svelte 5 runes**, no legacy `export let`/stores-with-`$:`. Cross-component state is a
  `.svelte.ts` store. Use `useTerm()` for the word for people (`freshie`/`freshies`), never
  a hardcoded one in user-facing text.
- **Styling is Tailwind v4 utility classes over the theme tokens** in `app.css`; no new
  colours, and dark mode must keep working.

## Recipes

**New screen.** Build it in `desktop/src/lib/screens/<name>/`, add a thin
`desktop/src/routes/<name>/+page.svelte`, then register it in `navItems()` in
`desktop/src/lib/shell/nav.ts` (icon from `shell/icons.ts`, which is the only icon set
screens reference). Add palette entries if it has actions. New data comes from
`queries.ts`, not from a `fetch` in the component.

**New IPC command.** Implement it in the owning `desktop/src-tauri/src/commands/<module>.rs`
as `#[tauri::command] #[specta::specta]` returning `Result<T, AppError>`, list it in
`builder()` in `commands/mod.rs`, run `pnpm bindings`, commit the regenerated
`bindings.ts`, and update `desktop/src-tauri/tests/ipc_contract.rs`. If it mutates, emit a
domain event and add the invalidation in `events.ts`. If it returns a domain type, decide
whether it needs a `commands/dto.rs` mirror (core cannot depend on `specta`) — prefer
returning the DTO shape the screen needs.

**New migration.** `migrations/000N_<topic>.sql`, next number, with a test in
`crates/weasel-store/tests/`. Read it back through a `Store` method, not ad-hoc SQL in a
command.

**New import format.** Add a reader next to `crates/weasel-import/src/read/{csv,sheet}.rs`,
add the variant to `SourceKind` in `crates/weasel-core/src/enums.rs`, and register the
reader in `READERS` in `crates/weasel-import/src/source.rs`. Put fixtures in
`crates/weasel-import/tests/fixtures/` — `.gitignore` allows spreadsheets and CSV only
there.

## Tests

- **Rust**: integration tests per crate in `crates/*/tests/`. Build a database with
  `TempDb` from `weasel-testkit`; inject the fake clock and clipboard rather than
  touching the OS. Reconciliation and normalisation get proptest coverage where a
  property is cheap to state.
- **Frontend**: `vitest` files next to the code (`*.test.ts`, `*.svelte.test.ts`). Test
  the logic that can be wrong — list-param parsing, formatting, wizard state, palette
  filtering — not the wiring.
- **No UI e2e suite.** The webview is not scriptable from Playwright; verify screens by
  running `pnpm dev:demo` and using them. `tauri-driver` is the intended long-term fix.
- **Do not test the bindings.** They are generated; the contract test and the CI diff
  check cover them.

## Data hygiene

- Never commit rosters, databases, backups or exports. `.gitignore` blocks `*.db`,
  `*.sqlite3`, `*.bak`, `*.xlsx/*.xls/*.ods/*.csv` outside `**/tests/fixtures/`, and
  `.dev-data/`. Add files by name; do not `git add .` or `git add -A`.
- Real user data lives in the platform data directory (`~/Library/Application
  Support/dev.weasel.app` on macOS); demo data lives in `.dev-data/`.
- When a command fails, its message reaches the log and the UI — never put an email
  address in it unredacted.
