# 08-exec — GUI MVP

Gets the shortest path to a usable GUI: the four screens that replace the TUI, built on
the IPC surface 04-exec shipped. It deliberately **replaces** the remaining phase plans
(06 phases 3–7) with an MVP cut. Everything not listed under §1 "In" is deferred, not
designed here. Read it fully, work the milestones in order, end each one green and in
one commit.

- **Written:** 2026-11-11, after 04-exec landed (`035fed0` on `rewrite/04-ipc`).
- **Effort:** ≈4.5 ideal days. Covers the UI half of 06 phases 3–4 in reduced form.
  Phases 5–7 become a post-MVP backlog (§9).
- **Read first:** this file · 04-exec §9 (handoff: error-code mapping, wizard flow,
  send flow) · 08-ui-design §Screen specifications (for intent only; §8 lists where this
  plan departs) · `desktop/src/lib/ipc/{client.ts,queries.ts,events.ts}`.

## 0. Ground rules (why this plan is short)

- **Architecture is frozen.** No new crates, ports, layers, generators, registries or
  abstractions. Each screen is a SvelteKit route plus a few components.
- **Rust is untouched** unless a bug blocks a screen. Then make a minimal fix with one
  test, run `pnpm bindings`, and log it in §10.
- Screens call `commands.*` through `call()` and read through the `*Queries` factories
  in `queries.ts`. There is no `invoke` outside `lib/ipc` (the eslint rule stays).
- **Never invalidate queries by hand after a mutation.** Every write emits a domain
  event, and `events.ts` already invalidates the right keys.
- Error handling everywhere: `VALIDATION` → `fieldError(err, field)` next to the input
  (otherwise a toast) · `CONFLICT` → toast "Changed elsewhere, reloaded" (the event
  already refetched) · anything else → `toast.ipcError(err)`.
- Tests: vitest only for pure TS logic (formatters, URL params, wizard state). UI is
  verified by the §7 manual smoke run against the real app. Rust tests must stay green.
- Never commit real roster data. The untracked root notebooks are not ours: never stage
  them (use `git add <paths>`).

## 1. Scope

**In:** cut the frontend registry, mock and Playwright · seeded dev workflow · Freshies
(list, detail, add, edit, remove/restore/archive) · Campaigns (list, detail, create,
edit, activate/archive, audience tabs, prepare send) · Send history (list, detail,
re-copy, note) · Imports (list, detail, discard draft, wizard with per-row conflict
decisions) · minimal dashboard · static nav and palette.

**Out (post-MVP backlog, §9):** conflict queue/bulk actions · import undo · activity
timeline · import progress bar · drag-and-drop import · `plan_rows` paging in the review
step · global search in the palette · bulk selection · contact/campaign hard delete in
the UI · column visibility/density · a11y/axe gates, 50k perf, FTS5 · diagnostics
screen · signing, updater, 3-platform installers · backend feature descriptors and
`app_features` cleanup.

## 2. Ask the owner before M0 (defaults in bold)

1. **Branch:** **`rewrite/08-mvp` stacked on `rewrite/04-ipc`, one commit per
   milestone, no pushes.**
2. **Default import scope:** **`delta`** (add/update only, like the TUI). `roster` (marks
   missing people removed) is an explicit choice, with the removals listed before apply.
   Needed by M4.
3. **Delete the Playwright suite with the mock:** **yes.** It can only run against the
   mock, because the Tauri webview cannot be scripted. Reintroduce e2e after the MVP
   with `tauri-driver` if it's wanted.

## 3. Versions

No new npm or cargo dependencies. Missing UI primitives are added with the installed
`shadcn-svelte` CLI (`^1.8.0` in `desktop/package.json`, `components.json` exists):
`pnpm -C desktop exec shadcn-svelte add table tabs select sheet label checkbox`. Commit
what it generates and nothing else. `@playwright/test` is removed in M0.

## 4. Facts that will bite

1. **Seed path vs. dev path.** Debug builds resolve the database at
   `<WEASEL_DATA_DIR>/dev/weasel.sqlite3` (`DataPaths::resolve(…, dev = true)`), but
   `seed-demo --data-dir D` writes `D/weasel.sqlite3`. Seed into `.dev-data/dev` (M0).
   `.dev-data/` is already gitignored.
2. **Response vs. request types.** Commands return `*_Serialize` types and take
   `*_Deserialize` requests. The bare `ContactDto` etc. are unions; don't annotate
   component props with them.
3. **Campaigns are created as `draft`,** and `campaigns_prepare_send` on a draft
   returns `INVALID_STATE`. Transitions: draft→active|archived, active→archived,
   archived→active (`can_transition`). The create dialog creates and then activates
   (M2).
4. **`expectedCount` must match what the user saw.** Pass `stats.unsent` from the stats
   query rendered in the confirm dialog. Refetch it when the dialog opens. `CONFLICT`
   means "the audience changed, review again": keep the dialog open with the refreshed
   numbers.
5. **The backend writes the clipboard.** `campaigns_prepare_send` and
   `send_batches_recopy` copy in Rust before committing, so success means it was
   copied. The UI never writes the clipboard itself.
6. **The picked-file allowlist is per process.** A draft import cannot be re-previewed
   after a restart, and `ImportBatchDetailDto` has no conflicts. So drafts in the list
   offer **Discard** only. To redo an import, start a new one (M4).
7. **Conflict decisions are per row.** Every row in every `conflict.rows` needs exactly
   one `ConflictResolutionDto {row, action, contactId?}`. Otherwise
   `INVALID_STATE "N rows need a decision"`; a duplicate row → `VALIDATION`.
   `linkContact` requires `contactId` from `conflict.candidates`. Choosing `createNew`
   for both rows of a `duplicateEmailInSource` fails at apply; show the error and let
   the user change the choice.
8. **Re-preview piles up drafts** unless `replacesBatchId` is the current draft's
   `batch.id` (M4 wizard state).
9. **CI references the things M0 deletes:** the `pnpm test:e2e`/playwright steps, the
   generator job (`pnpm gen feature demo …`), and the root `test` script's
   `node --test tools/*.test.mjs`. Remove them in the same commit, or CI breaks.
10. **Registry consumers:** `AppShell`, `Sidebar` (`NavEntry`), `CommandPalette`
    (`PaletteEntry`) + its test, `HelpView`, `SettingsView`, `DashboardView`,
    `tests/boundaries.test.ts`. All must be rewired or deleted in M0.
11. **`+layout.ts`** installs the mock in `load`. Delete the `load` function but keep
    `ssr = false` and `prerender = false`.

## 5. Target additions

```text
DELETE  desktop/src/lib/ipc/{mock.ts,mock-data.ts,mock-domain.ts,mock.test.ts,mock-domain.test.ts}
DELETE  desktop/src/lib/features/** (registry*, types.ts, manifests, READMEs; components move)
DELETE  desktop/src/routes/dev/** · desktop/tests/{e2e/**,boundaries.test.ts} · desktop/playwright.config.ts
DELETE  tools/gen-feature.mjs · tools/gen-feature.test.mjs
EDIT    package.json, desktop/package.json, .github/workflows/ci.yml, desktop/src/routes/+layout.ts
ADD     desktop/src/lib/shell/nav.ts
MOVE    lib/features/{dashboard,settings,help}/components/* → lib/screens/{dashboard,settings,help}/
ADD     desktop/src/lib/components/ui/{table,tabs,select,sheet,label,checkbox}/†   (shadcn CLI)
ADD     desktop/src/lib/components/app/{Pager,SearchInput,StatusBadge,EmptyState,ConfirmDialog}.svelte
ADD     desktop/src/lib/utils/{format.ts,list-params.ts} (+ tests)
ADD     desktop/src/routes/freshies/{+page.svelte,[id]/+page.svelte}       + lib/screens/freshies/*
ADD     desktop/src/routes/campaigns/{+page.svelte,[id]/+page.svelte}      + lib/screens/campaigns/*
ADD     desktop/src/routes/history/{+page.svelte,[id]/+page.svelte}        + lib/screens/history/*
ADD     desktop/src/routes/imports/{+page.svelte,[id]/+page.svelte,new/+page.svelte} + lib/screens/imports/*
ADD     desktop/src/lib/screens/imports/wizard.svelte.ts (+ wizard.test.ts)
ADD     docs/adr/0007-mvp-cut.md
```

## 6. Milestones

### M0 — Cut and dev loop (≈0.5 d)

- Resolve §2. Branch.
- **Mock:** delete the five mock files, the `load` in `+layout.ts` (§4.11), the
  `dev:mock` script (desktop) and `dev:web` (root).
- **Playwright:** delete `playwright.config.ts` and `tests/e2e/`, the `test:e2e`
  scripts (both `package.json`s), the `@playwright/test` devDependency, and the two CI
  steps (§4.9). Run `pnpm install` to update the lockfile.
- **Registry and generator:** delete `lib/features/**`, `routes/dev/**`,
  `tools/gen-feature*`, the root `gen` script, the CI generator job, and the
  `node --test tools/*.test.mjs` half of the root `test` script.
  Delete `tests/boundaries.test.ts`; the eslint `no-restricted-imports` rule still keeps
  Tauri out of components.
- **`lib/shell/nav.ts`** replaces the registry:
  ```ts
  export interface NavItem { label: string; href: string; icon: IconName; shortcut: string }
  export const NAV: NavItem[] = [
    { label: 'Dashboard', href: '/',          icon: 'dashboard', shortcut: 'g d' },
    { label: 'Freshies',  href: '/freshies',  icon: 'users',     shortcut: 'g f' },
    { label: 'Campaigns', href: '/campaigns', icon: 'mail',      shortcut: 'g c' },
    { label: 'History',   href: '/history',   icon: 'history',   shortcut: 'g h' },
    { label: 'Imports',   href: '/imports',   icon: 'upload',    shortcut: 'g i' },
    { label: 'Settings',  href: '/settings',  icon: 'settings',  shortcut: 'g s' },
    { label: 'Help',      href: '/help',      icon: 'help',      shortcut: '?'   }
  ];
  export interface PaletteEntry { id: string; title: string; keywords?: string[]; shortcut?: string; href: string }
  export const PALETTE: PaletteEntry[]  // "Go to X" per NAV item + "Import list" → /imports/new,
                                        // "New campaign" → /campaigns?new=1, "Add freshie" → /freshies?new=1
  ```
  Help moves from `g h` to `?` (History takes `g h`). Remove the `code` and `box` icons
  if nothing uses them.
- Rewire: `Sidebar` takes `NAV` · `AppShell` builds shortcut bindings from `NAV`/`PALETTE`
  (`mod+k` stays) and palette `run` = `goto(resolvePath(entry.href))` · `CommandPalette`
  takes `PaletteEntry[]` (update its test) · `HelpView` lists `NAV`/`PALETTE` shortcuts ·
  `SettingsView` renders `GeneralSettings` directly · `DashboardView` renders
  `AppStatusWidget` directly. Move these components to `lib/screens/<name>/` and import
  them from the routes.
- **Dev loop:** add a root script
  `"dev:demo": "pnpm seed:demo --data-dir .dev-data/dev --force && WEASEL_DATA_DIR=\"$PWD/.dev-data\" pnpm dev"`
  (§4.1). Document it in `desktop/README.md` as *the* way to work on screens.
- Add the shadcn primitives (§3).

Done when: `pnpm verify` is green, and `pnpm dev:demo` opens the window with a sidebar
of seven entries and the dashboard reporting the seeded DB as healthy.
Commit: `refactor(desktop): drop feature registry, IPC mock and e2e for the MVP`

### M1 — List kit and Freshies (≈1 d)

- `utils/format.ts`: `formatDate(ms)` (local short date), `formatDateTime(ms)`,
  `fullName({firstName,lastName})`, `plural(n, 'freshie')`. Test them.
- `utils/list-params.ts`: `readListParams(url, defaults) → {search, page, status, …}`
  and `listHref(url, patch) → string` (resets `page` to 1 when anything else changes).
  Lists keep `search`, `status` and `page` in the URL so back from a detail page returns
  to the same list. Page size is fixed at 50. Test the round trip.
- `components/app/`: `Pager` (`{total, offset, limit}` → "1–50 of 1,284" + prev/next
  links) · `SearchInput` (debounced 250 ms, `Esc` clears, `mod+f` focuses) ·
  `StatusBadge` (text + colour for contact/campaign/import/send statuses) · `EmptyState`
  (title, text, optional action) · `ConfirmDialog` (title, body, confirm label,
  destructive flag, async `onConfirm` with a pending state).
- `/freshies`: columns name, email, status, added · search · status tabs *Active*
  (default) / *Removed* / *Archived* / *All* (`statuses` array) · row click →
  `/freshies/[id]` · "Add freshie" (also opened by `?new=1`): dialog for first name,
  last name, email and notes → `contacts_create`, then navigate to the new contact.
  Empty state points to Import.
- `/freshies/[id]` (a page, not a drawer): edit form prefilled from `contacts_get`; save
  sends `contacts_update {id, version, …changed fields only}` · email history (email,
  from–to, reason) · campaigns received (campaign link, batch link `/history/[batchId]`,
  date) · actions by status: active → *Mark removed* / *Archive*; removed/archived →
  *Restore* (`contacts_set_status`, each through `ConfirmDialog`).

Done when: `pnpm verify` is green; smoke steps 1–3 (§7) pass in `pnpm dev:demo`.
Commit: `feat(desktop): freshies list and detail`

### M2 — Campaigns and prepare send (≈1 d)

- `/campaigns`: columns name, status, progress `sent/total` (a bar plus text), last
  send, updated · status tabs *All* (default) / *Active* / *Draft* / *Archived* ·
  search · "New campaign" (also `?new=1`): dialog for name and description →
  `campaigns_create`, then `campaigns_set_status {status:'active'}` (§4.3), then
  navigate to the detail page. If activation fails, the detail page shows the draft
  with an *Activate* button.
- `/campaigns/[id]`: header with name, status and actions: *Edit* (dialog → `campaigns_update`
  with `version`) · *Activate* for draft/archived · *Archive* for active (confirm) ·
  *Prepare send*. Stat row from `campaigns_stats`: total, sent, unsent, last send.
  Tabs: *Not sent* (default) / *Sent* / *All* → `campaigns_audience {filter:
  'unsent'|'sent'|'all', search, limit 50, offset}` with search and pager ·
  *History* → `send_batches_list {campaignId}` linking to `/history/[id]`. The
  description is shown under the header (there is no Notes tab).
- **Prepare send** (`PrepareSendDialog.svelte`): refetch stats on open (§4.4) · shows
  "Copy N addresses and record a send batch", the first 10 unsent names, and an
  explanation that a history entry is created · disabled with a reason when unsent = 0
  or status ≠ active · confirm → `campaigns_prepare_send {campaignId, expectedCount:
  stats.unsent}` → close, toast `Copied ${recipientCount} addresses` with action
  *View batch* → `/history/${batchId}` · `CONFLICT` → keep the dialog open, show
  "The audience changed. Check the new count and confirm again", refetch stats ·
  `IO` + `retryable` → error toast with *Retry*.

Done when: `pnpm verify` is green; smoke steps 4–6 pass.
Commit: `feat(desktop): campaigns and prepare send`

### M3 — Send history (≈0.5 d)

- `/history`: newest first; columns campaign, date, recipients, note · search (backend
  matches campaign, recipient and note) · optional `?campaign=` filter shown as a
  removable chip.
- `/history/[id]`: header with campaign link, date, count · *Copy again* →
  `send_batches_recopy {id}` → toast `Copied N addresses again` · note: textarea +
  *Save* → `send_batches_set_note` (the `VALIDATION` field `note` appears inline) ·
  recipients → `send_batches_recipients` (snapshot name/email, search, pager). No delete.

Done when: `pnpm verify` is green; smoke step 7 passes.
Commit: `feat(desktop): send history and re-copy`

### M4 — Imports and wizard (≈1.25 d)

- `/imports`: columns source name, kind, date, status badge, added/updated/removed/
  conflicts · "Import list" → `/imports/new` · draft rows: *Discard* (confirm →
  `imports_discard`) only (§4.6) · row click → `/imports/[id]`.
- `/imports/[id]`: summary counts + members (`imports_get {batchId, limit, offset}`)
  with pager; a draft shows a *Discard* button and a note to start a new import.
- `lib/screens/imports/wizard.svelte.ts`: a class `ImportWizard`, constructed with the
  `commands` object (injectable for tests), holding
  `step: 'source'|'columns'|'review'|'done'`, `source`, `inspection`, `sheet`,
  `mapping`, `scope` (default per §2.2), `preview`, `decisions: Map<row, Resolution>`,
  `result`, `busy`, `error`. Methods:
  `pick()` (pick_source; `null` → stay; then `inspect`) · `inspect(sheet?)` (prefill
  `mapping` from `suggestedMapping`; any missing column stays unset) ·
  `runPreview()` (requires all three columns; passes `replacesBatchId: preview?.batch.id`,
  §4.8) · `back()` (review → columns, keeps the draft id) ·
  `decide(row, resolution)` · `get undecidedRows()` (rows of every conflict without a
  decision, §4.7) · `apply()` (blocked while `undecidedRows.length > 0`; sends
  `resolutions` = every decision) · `discard()`. All UI logic lives in the class, and
  components only render it.
- `/imports/new` renders one step at a time:
  1. **Source:** "Choose file…" → `pick()`; shows name, kind, size.
  2. **Columns:** a sheet select (only when `sheets.length > 1`, re-inspects) · three
     selects (first name, last name, email) over `headers` · the first 5 `sampleRows`
     as a table · scope radio with one-line explanations ("Add and update only" /
     "This file is the complete list: people missing from it are marked removed") ·
     *Preview*.
  3. **Review:** `summary` counts (`added`, `updated`, `removed`, `conflicts`,
     `unchanged`, `invalid`) · `warnings` as callouts (`largeRemoval` is shown in
     red) · `errors` (row, field, reason) · `removals` (names, emails) · conflicts: per
     conflict its kind in plain words, per row a select *Link to {candidate}* … /
     *Create new* / *Skip* · `sample` table · *Back*, *Discard* (confirm), *Apply*
     (disabled with "N rows need a decision").
  4. **Done:** applied summary, links to `/imports/[id]` and `/freshies`.
  A spinner with "Reading file…" or "Applying…" while `busy` (no progress events in the
  MVP). Leaving the page keeps the draft.
- `wizard.test.ts` (stub commands): pick cancelled → stays on `source` · inspect with
  no email suggestion → preview refused · second preview passes the first draft id as
  `replacesBatchId` · apply refused while a duplicate-email conflict has one undecided
  row · apply sends one resolution per conflict row.

Done when: `pnpm verify` is green; smoke steps 8–10 pass.
Commit: `feat(desktop): import list, detail and wizard`

### M5 — Dashboard, docs, smoke (≈0.25 d)

- Dashboard: stat cards (active and removed freshies from `contacts_stats`, campaigns =
  `campaigns_list {limit:1}.total`, last import from `imports_list {limit:1}`, last send
  from `send_batches_list {limit:1}`) · quick actions Import list / New campaign / Add
  freshie · keep `AppStatusWidget`.
- `docs/adr/0007-mvp-cut.md`: drop the frontend feature registry, generator, IPC mock
  and Playwright. Consequences: features are routes; UI is verified manually until
  tauri-driver e2e; backend descriptors stay unused. It partly supersedes ADR 0004.
  Add it to `docs/adr/README.md`.
- `06-delivery-plan.md`: a note at the top saying that phases 3–7 are superseded by
  08-exec for the MVP, with the backlog in 08-exec §9. `05-extensibility.md`: a note at
  the top pointing to ADR 0007. `09-parity-checklist.md`: tick the phase 3/4 items the
  smoke run proved, with evidence "manual smoke <date>, 08-exec §7 step N"; mark items
  moved to the backlog as "(post-MVP)". `README.md` status: "GUI MVP — see 08-exec.md".
- Run §7 end to end; fill §10.

Commit: `docs: GUI MVP executed; ADR 0007; parity evidence`

## 7. Final acceptance — manual smoke run (`pnpm dev:demo`)

1. Freshies lists seeded contacts; search narrows them; page 2, then a detail, then
   back returns to page 2.
2. Add a freshie; it opens on its detail page; a duplicate email shows an inline error.
3. Edit an email → a second history row appears; mark removed → it leaves the Active
   tab; restore it.
4. Create a campaign → it is active, with unsent = active freshies.
5. Prepare send → the clipboard holds exactly N addresses (paste to check); the toast
   links to the batch; unsent is now 0 and *Prepare send* is disabled with a reason.
6. Archive the campaign → *Prepare send* explains why it is disabled; *Activate*
   brings it back. (The "audience changed" path can't be triggered from one window
   with a modal open; the backend side is covered by
   `campaigns::tests::wrong_reviewed_count_leaves_no_batch`.)
7. History lists the batch; *Copy again* copies the same addresses after editing one
   recipient's email (snapshot); a note saves.
8. Import a synthetic CSV (in the scratch dir, never committed) with renamed headers →
   map the columns → preview → apply → the new people appear in Freshies.
9. Re-import the same file → all unchanged and a "same file" warning shown; discard →
   the draft is gone from Imports.
10. Import a sheet with one duplicate email → Apply is disabled until both rows have a
    decision; skip one, link or create the other → apply succeeds.

Plus: `pnpm verify` green · `pnpm bindings && git diff --exit-code` clean (Rust
untouched) · CI green or explicitly unverified.

Ticks (09): phase 3 — Freshies list/search/filters (URL state), contact detail,
campaigns list/detail, audience tabs, send history and batch detail, imports
list/detail · phase 4 — create/edit/archive campaign, prepare-send exactness, one
batch per send, re-copy snapshot, import wizard mapping/preview, idempotent re-import,
discard, manual create/edit, remove/restore. Not ticked (backlog): global palette
search, activity timeline, every reconciliation item beyond per-row conflicts, all of
phases 6–7.

## 8. Deviations from the planning docs

| # | Doc says | This plan | Why |
| --- | --- | --- | --- |
| 1 | Sidebar/palette/settings/dashboard composed from feature manifests (05, ADR 0004) | Static `nav.ts`; screens are routes | One developer, ~6 screens; the registry was cost without a consumer |
| 2 | `pnpm dev:web` mock + Playwright e2e (04-exec M5, 07) | Deleted; manual smoke on the real app with a seeded DB | The mock duplicated the backend in TS and every command change had to be mirrored |
| 3 | Contact detail as a drawer `/freshies?id=` | Page `/freshies/[id]` | Simpler routing and back navigation |
| 4 | Wizard steps Source/Columns/Scope/Review/Apply | Scope merged into Columns; Apply is a button | Fewer steps, same decisions |
| 5 | Progress bar for import phases | Spinner | Files are small; progress events are kept for later |
| 6 | Conflict queue with bulk actions | One select per conflicting row in Review | Minimum needed so conflicting sheets can be imported |
| 7 | Draft imports resumable | Discard only | The picked-file allowlist is per process (§4.6) |
| 8 | Campaign tabs Audience/Send history/Notes | Not sent/Sent/All/History; description under the header | Fewer tabs |
| 9 | Hard delete exposed (04-exec §2.2) | Not in the UI | MVP; archive/remove cover it |
| 10 | Keyboard model: `g h` = Help, `mod+n`, `mod+shift+i`, density, column visibility | `g h` = History, `?` = Help; the rest deferred | MVP |
| 11 | 06 phases 3–7, 1.0 definition of done | Replaced by this MVP; backlog in §9 | Owner decision 2026-11-11 |
| 12 | §3: add table, tabs, select, sheet, label, checkbox | Kept table and label only | Status tabs are links (`FilterTabs`, URL state); native `<select>` for mapping and conflicts |
| 13 | §5: `wizard.test.ts` | `wizard.svelte.test.ts` | Runes need the happy-dom project, which matches `*.svelte.test.ts` |
| 14 | §1: no Rust or IPC changes | `keys.imports.detail` now takes the request | The key ignored paging, so member page 2 showed cached page 1 (TS-only fix) |
| 15 | — | eslint `svelte/no-navigation-without-resolve` ignores links | Runtime hrefs go through `href()`/`go()` in `utils/paths.ts`, which apply the base |

## 9. Post-MVP backlog (ordered by expected value)

1. Global search in the palette (`search_global` exists) · `mod+n` per screen.
2. Activity timeline on contact/campaign detail (`activity_list` exists).
3. Import progress bar (`onEvent` + `importProgress`) · drag-and-drop with a Rust-side
   allowlist.
4. Import undo, configurable identity columns, bulk conflict actions (06 phase 5).
5. Packaging for the owner's own machine (unsigned macOS build) before any signing or
   updater work.
6. tauri-driver e2e for the §7 smoke path.
7. Optional simplification: derive `specta::Type` on core types and delete DTO
   mirrors (needs an ADR that relaxes the "domain has no framework deps" rule);
   delete the backend `FeatureDescriptor`s and `app_features`.

## 10. Execution log

| Date | Milestone | Note |
| --- | --- | --- |
| 2026-11-11 | M0 | `cbb0d1b` registry, generator, mock, Playwright and boundary test removed (−3.3k lines); `nav.ts`; `pnpm dev:demo`; the plan doc itself is `a5b4d2f` |
| 2026-11-11 | M1 | `8ae456b` list kit (`components/app/*`, `utils/{format,list-params,paths,errors}.ts`) and Freshies list/detail/add |
| 2026-11-11 | M2 | `358582c` campaigns list/detail, create (activates the draft), edit, archive, prepare send with `expectedCount` |
| 2026-11-11 | M3 | `1695d9d` send history list/detail, copy again, note |
| 2026-11-11 | M4 | `86d3066` imports list/detail/discard and the wizard (`ImportWizard`, 5 tests); imports detail key fix (§8.14) |
| 2026-11-11 | M5 | Dashboard stats and quick actions; ADR 0007; notes in 05/06/09/README; unused primitives removed (§8.12) |
| 2026-11-11 | §7 | `pnpm verify` green. **Manual smoke not run by the executing agent**: it could not see the Tauri window. 09 UI items stay unticked until the owner runs §7 |
