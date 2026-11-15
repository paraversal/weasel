# HANDOFF

State of Weasel after the rewrite, and where development can go next.
Last updated 2026-11-15, after the cleanup commit (`34e4177` is the last commit that
still contained the TUI and the planning docs).

Read [`AGENTS.md`](AGENTS.md) for how to work here. This file is the *what* and *why*.

## Where things stand

The Textual TUI has been replaced by a Tauri 2 + SvelteKit desktop app. The GUI does
every workflow the TUI did: import a roster, fix conflicts, edit people, build a
campaign, prepare a send to the clipboard, and re-copy a past batch. The application
code is ~29k lines; the frontend is roughly a third of it.

| Screen (route) | What it does |
| --- | --- |
| Dashboard (`/`) | contact/campaign/send counts, recent activity, resolved data paths, open folder, take a backup |
| Contacts (`/contacts`) | list with server-side search, sort, status/source/import filters; add, edit, remove, restore, archive |
| Contact detail (`/contacts/[id]`) | email history, campaigns received, activity, invalid-address hold and override |
| Campaigns (`/campaigns`) | list, create/edit (merge tags), activate/archive; detail has sent/unsent/all audience tabs and **Prepare send** |
| History (`/history`) | send batches newest first, search by recipient/note; detail shows the snapshotted recipients and re-copies the same addresses |
| Imports (`/imports`) | batch list and detail; wizard maps columns, previews add/update/remove/no-op, records per-row conflict decisions, discards a draft |
| Settings (`/settings`) | the word used for people (freshie/freshies), invalid-address rules, email check settings |
| Help (`/help`) | keyboard map and the workflow notes |

Cross-cutting: `⌘K` palette, `g d`/`g p`/`g c`/`g h`/`g i`/`g s` shortcuts, light/dark
theme, toasts, drag a spreadsheet anywhere onto the window to start an import.

Data: SQLite, schema version **3** (`migrations/0001_app.sql` … `0003_email_checks.sql`),
WAL, foreign keys on, `VACUUM INTO` backup before any migration. Tables:
`settings`, `app_meta`, `import_batches`, `contacts`, `contact_email_history`,
`campaigns`, `send_batches`, `send_records`, `audit_events`, `email_flags`,
`invalid_email_rules`.

## What this cleanup removed

| Removed | Why | How to get it back |
| --- | --- | --- |
| `legacy/python-tui/` (~1.1k lines) | the rewrite is finished; nothing imports it | `git show 34e4177:legacy/python-tui/src/db.py` |
| `docs/adr/`, `docs/rewrite/` (~5.2k lines) | planning artifacts for a finished rewrite | `git show 34e4177:docs/rewrite/08-exec.md` |
| `csp.ipynb`, `test.ipynb` | scratch prototypes; see the room feature below | never committed; the room feature below restates them |

The decisions worth keeping are restated in `AGENTS.md` and in the next section rather
than in ADRs. If a future change needs to revisit one, write a short note here instead of
resurrecting the ADR directory.

## Decisions that are load-bearing

- **Rust owns the data logic, Svelte only renders.** `weasel-core` is pure (no Tauri, no
  SQLite); `weasel-store` owns SQL; Tauri commands are a thin adapter. Enforced by
  `tools/check-layers.sh`.
- **One typed IPC surface.** Commands are generated into
  `desktop/src/lib/ipc/bindings.ts` by tauri-specta; CI fails if the committed bindings
  are stale. A backend change breaks the frontend build instead of failing at runtime.
- **No legacy data import.** The app never discovers or migrates a TUI database; users
  re-import their roster. A developer tool outside the app,
  `cargo run -p weasel-testkit --bin migrate-legacy -- --from <weasel_lair.db> --data-dir <dir>`,
  can copy a TUI database into a fresh one once.
- **Screens are plain SvelteKit routes.** The earlier feature registry, generator and
  IPC mock were deleted as unnecessary machinery for one developer and a handful of
  screens. Navigation is `desktop/src/lib/shell/nav.ts`.
- **Clipboard sending, not SMTP.** "Prepare send" writes addresses to the clipboard and
  records an immutable snapshot per recipient; history survives email changes.
- **Soft removal everywhere.** Contacts move `active → removed → archived`; nothing is
  hard-deleted from the UI, and send records are immutable.
- **Every write emits an event** and the frontend invalidates query keys from
  `desktop/src/lib/ipc/events.ts`; never invalidate by hand after a mutation.

## Known gaps

- **No automated UI test.** The Tauri webview cannot be scripted by Playwright, so the
  suite was dropped. Screens are verified by hand against `pnpm dev:demo`. `tauri-driver`
  e2e is the intended fix.
- **Untested at scale.** The 50k-contact p95 target and the 10k-row import
  responsiveness were never measured on the target machine.
- **Unsigned, unbundled.** No installer, signing, notarisation or updater. CI builds a
  debug bundle without a bundle step.
- **Accessibility was never audited** (keyboard paths exist, contrast and screen-reader
  passes do not).
- **DTO mirrors.** `desktop/src-tauri/src/commands/dto.rs` restates core types because
  the domain may not depend on `specta`. Deriving `specta::Type` on core types would
  delete the mirrors, at the cost of that rule.
- **Unused backend descriptors.** `feature_descriptors()` and the `app_features` command
  remain from the registry era with no frontend consumer.

## Suggested next steps, in order

1. **Package for the owner's machine** — an unsigned macOS build — so the app can be used
   for a real import → campaign → send cycle. Real use will find more than any backlog
   item will.
2. **Rooms screen** (specified below) — the one feature that was being prototyped when
   the rewrite ended.
3. **`tauri-driver` e2e** for the smoke path, before the UI grows further.
4. Then, in rough value order: global search in the palette (`search_global` already
   exists), activity timeline on contact/campaign detail (`activity_list` exists), import
   progress bar (`importProgress` event exists), import undo, configurable identity
   columns, bulk conflict actions.

## Not implemented: assigning alphabetical ranges to rooms

The two deleted notebooks were a prototype for this. Nothing in the app implements it;
this section is the spec so it does not have to be rediscovered.

**Product intent.** Rooms have a fixed occupancy. Students are split into contiguous
alphabetical ranges of last names, one range per room, so that a person can be told
"you are in 22/215, letters A–C". The screen should let the user produce that split,
see how full each room is, and adjust it.

**What the prototype did** (`test.ipynb`, `csp.ipynb`):

- Read the student roster export (`.xlsx`) with pandas
  (`read_excel(..., header=1, skipfooter=1)`), using columns `NACHNAME` (last name) and
  `ABSCHLUSS` (degree; `"Master of Science"` split from everyone else).
- Bucketed the first letter of the last name, then

  ```python
  sorted_letters = sorted(letters)          # one entry per student, not per letter
  base, remainder = divmod(len(sorted_letters), n)
  # group i gets base + (1 if i < remainder else 0) consecutive entries
  # label = f"{group_slice[0]}-{group_slice[-1]}"
  ```

  so groups differ by at most one student. Bachelor students were split into 5 groups,
  master students into 2, chosen by hand.
- Room list and capacities from `csp.ipynb`:

  | Room | Capacity |
  | --- | --- |
  | 15/113 | 55 |
  | 22/215 | 62 |
  | 22/106 | 40 |
  | 22/107 | 40 |
  | 12/422 | 32 |
  | 15/133 | 32 |
  | 15/115 | 32 |
  | **total** | **293** |

**Known flaws to fix, not port:**

1. It slices the *sorted list of students*, so a range boundary can fall inside one
   letter: two adjacent groups can both print `M-M`, and those people become ambiguous.
   Boundaries must land between distinct letters.
2. `n` is a magic number. Rooms and capacities are the real constraint; the group count
   and each group's size should fall out of filling rooms, and total capacity below the
   headcount must surface as an overflow warning instead of silently overfilling.
3. It counts every row in the table, including removed contacts. The screen should count
   active contacts in the current roster.
4. Nothing was persisted or exported.

**Design sketch (not a commitment).**

- Schema: `rooms(id, name UNIQUE, capacity CHECK(capacity > 0), active)` and
  `room_assignments(scope, room_id, letter_from, letter_to)` where `scope` is the import
  batch (or a term) so a re-import can recompute without losing the room list. Overlap
  between ranges is a service-level check, not an index.
- Core: a pure function in `weasel-core` that takes `[(initial, count)]` and
  `[(room, capacity)]` and returns either an assignment (one contiguous, non-splitting
  range per room, no range over its room's capacity) or a structured overflow. Greedy
  fill in letter order is probably enough; a DP over 26 letters is cheap if greedy
  leaves a bad tail. Unit-test it on the real distribution (letters are heavily skewed;
  a naive equal-count split will overfill the small rooms).
- IPC: `rooms_list`, `rooms_upsert`, `rooms_delete`, `room_plan`, `room_plan_save`.
  Bindings regenerated as usual.
- Screen: `/rooms` plus a line in `nav.ts`. Show the A–Z counts as a small histogram,
  the rooms with capacity and planned fill, a warning bar for overflow, the ability to
  drag a boundary between letters, and a "copy this room's addresses" action that reuses
  the send-batch clipboard path. Use `useTerm()` for the word for people.

**Open questions to settle before building:** is the assignment per import batch or per
term; do bachelor and master students share rooms; is a letter range genuinely required
(if people are assigned individually, no range is needed and the problem is a plain
capacity partition); and what happens to an assignment when a re-import changes the
counts.
