# 08 — UI and interaction design

The GUI should feel like a focused local tool, not a website. One persistent shell, fast
keyboard access, explicit destructive actions, and clear feedback for clipboard/import
operations.

## Information architecture

```text
Weasel
├── Dashboard            /
├── Freshies             /freshies            (contacts)
│   └── Detail drawer    /freshies?id=…
├── Campaigns            /campaigns
│   └── Detail           /campaigns/[id]
├── Send history         /history
│   └── Batch detail     /history?id=…
├── Imports              /imports
│   ├── Wizard           /imports/new
│   └── Batch detail     /imports/[id]
├── Settings             /settings
└── Help                 /help
```

The sidebar is generated from feature manifests. The top bar contains global search
(`Cmd/Ctrl+K`), a primary "Import list" action, and a theme/status menu.

## Screen specifications

### Dashboard

- Stat cards: active freshies, removed, campaigns, last import, last send.
- "Needs attention" card: draft imports, unresolved conflicts, campaigns with no sends.
- Recent imports (5) and recent send batches (5), each linking to detail.
- Quick actions: Import list, New campaign, Add freshie.
- Extensible: dashboard widgets are lazy-loaded feature components.

### Freshies

- Table: name, email, status, source, import batch, date added, campaigns sent.
- Server-side pagination, sort by last name (default), search across name/email.
- Filters: status (active/removed/archived), import batch, source (import/manual).
- Row click opens a right-side detail drawer:
  - editable name/email/notes;
  - email history with dates and reasons;
  - campaigns this person has received, with batch links;
  - mark removed / restore / archive actions.
- "Add freshie" opens a small dialog; created contacts have `source = manual` and are
  excluded from roster removals.
- Bulk selection supports mark removed/restore. There is no bulk hard delete.
- Empty state explains importing or adding manually.

```text
┌ Sidebar ─┬──────────────────────────────────────────────────────────────┐
│ Dashboard│ Freshies                        [Search…]  [Filters] [+ Add]│
│ Freshies │──────────────────────────────────────────────────────────────│
│ Campaigns│ ☐  Name              Email                 Status   Added   │
│ History  │ ☐  Ada Lovelace      ada@example.org       Active   12 Sep  │
│ Imports  │ ☐  Grace Hopper      grace@example.org     Active   12 Sep  │
│ Settings │ ☐  Alan Turing       alan@example.org      Removed  02 Sep  │
│ Help     │──────────────────────────────────────────────────────────────│
│          │ 1–100 of 1,284                                  ‹ 1 2 … 13 › │
└──────────┴──────────────────────────────────────────────────────────────┘
```

### Campaigns

- List: name, status, sent/total progress, last batch, updated. Create/edit/archive.
- Detail:
  - header: name, status, actions (Edit, Prepare send, Archive);
  - stat row: total audience, sent, unsent, last send;
  - tabs: **Audience** (sent / unsent / all), **Send history**, **Notes**.
- Audience tab is a paginated contact table with search, so the common question "who has
  not received this?" is one click away.
- "Prepare send" opens a confirmation dialog:
  - shows count and a sample of recipients;
  - explains that a new history entry will be created and emails copied;
  - disabled with an explanation when there are zero unsent recipients.
- After success: toast with recipient count and a "View batch" action; the audience
  refreshes via `sendBatchesChanged`.

### Send history

- List of batches, newest first: campaign, date, recipient count, note.
- Global search across campaign name, recipient name/email, and batch note.
- Batch detail: immutable recipient snapshots, copy status, "Copy again", edit note.
- Re-copy does not create a new batch; it reuses the same recipient set so the history
  stays truthful. (Creating a new batch is only done from "Prepare send".)
- No destructive delete. Batches are history.

### Imports

- List: source name, kind, date, status chip, counts (added/updated/removed/conflicts).
- Draft batches are resumable or discardable.
- Wizard steps:

```text
1. Source       2. Columns        3. Scope            4. Review           5. Apply
   pick file →    map headers  →    roster / delta  →   diff + conflicts →  progress/result
```

**1. Source:** native picker or drag-and-drop; shows file name, size, detected kind.

**2. Columns:** auto-detect `VORNAME`/`NACHNAME`/`EMAIL` (and common aliases); user maps
first name, last name, email; optional identity columns; a 5-row preview updates live.

**3. Scope:** roster (the sheet is the complete list) vs delta (add/update only), with a
plain-language explanation. Advanced: identity columns and "apply safe changes
automatically".

**4. Review:** summary counts and a filterable row table:

```text
Action       Count   Example
Add            114   New Person <new@example.org>
Update email     3   Ada Lovelace: old@… → new@…
Mark removed     2   Alan Turing
Conflict         1   "John Smith" matches 2 contacts
No change      116
```

Conflicts show source row, candidate contacts and actions: **Link to contact**, **Create
new**, **Skip**. Nothing is written until **Apply**.

**5. Apply:** progress bar for parsing/matching/applying, then a result summary with
"Undo this import" and links to affected people. Errors keep the draft intact for retry.

### Settings

- **General:** theme (system/light/dark), density, date format, confirmations.
- **Data:** database path, backup now, open data folder, import legacy database, export
  diagnostics.
- **Import defaults:** default scope, identity columns, column aliases.
- **Features:** flags with descriptions.
- **About:** versions, schema version, license, logs.

### Help

- Getting started, importing a roster, preparing a send, conflict resolution, keyboard
  shortcuts, glossary ("freshie", "campaign", "copy/batch", "roster").
- Searchable in place.

## Visual system

- Tailwind tokens with semantic names (`bg-surface`, `text-muted`, `border-subtle`,
  `accent`, `danger`, `success`, `warning`).
- Light/dark/system themes; no flash on launch (apply theme class before mount).
- shadcn-svelte-style primitives copied into `components/ui`: button, input, select,
  dialog, sheet, tabs, table, badge, toast, dropdown, command, tooltip, empty-state.
- Lucide icons; consistent 16/20 px sizing.
- Compact data-dense layout by default, with a comfortable density option.
- Status language: Active / Removed / Archived, Applied / Draft / Failed, Sent / Not
  sent. Never rely on colour alone; pair chips with text.

## Keyboard model

| Shortcut | Action |
| --- | --- |
| `Cmd/Ctrl+K` | Command palette (navigate, act, search) |
| `G` then `D/F/C/H/I/S` | Go to Dashboard/Freshies/Campaigns/History/Imports/Settings |
| `Cmd/Ctrl+Shift+I` | Import a list |
| `Cmd/Ctrl+N` | New item for the current screen |
| `Cmd/Ctrl+F` | Focus the current screen's search |
| `Cmd/Ctrl+Enter` | Primary action in a dialog/wizard step |
| `Esc` | Close dialog/drawer, clear search |
| `?` | Shortcut help |

Shortcuts are registered from feature manifests, with conflict detection at startup and
a diagnostics warning. Typing inside inputs never triggers bare-letter navigation.

## Interaction rules

1. **Destructive actions are explicit.** Archive/remove, discard import and undo each
   have a labelled confirmation. There is no "delete all".
2. **Clipboard feedback is honest.** Show what was copied, how many addresses, and a
   failure if the OS refused. Never claim success before the write resolves.
3. **History is read-only.** Past send batches show snapshotted names/emails and cannot
   be edited except for their note.
4. **Drafts are safe.** Leaving the import wizard keeps the draft; discarding is explicit.
5. **URLs carry list state.** Search, filters, sort and page are query parameters so
   reload/back/forward work and links are shareable inside the app.
6. **Loading is local.** Skeletons per pane, not a blocking full-screen spinner; imports
   and other long operations have progress indicators.
7. **Errors explain the next step.** `CONFLICT` offers reload/compare; `VALIDATION`
   focuses the field; `IO` offers the data folder; `INTERNAL` offers diagnostics.

## Responsive behaviour

The primary target is a desktop window (minimum ~960×600). Below 1100 px the sidebar
collapses to icons with a tooltip; detail drawers become full-height sheets. Tables keep
horizontal scrolling rather than hiding essential columns. Multi-window use is supported
by the backend event model, not by sharing frontend state.

## UI acceptance checks

- Every TUI workflow has a labelled GUI path and a keyboard path.
- No screen shows more than 500 rows at once; large lists are paginated/virtualized.
- Removal, discard, undo and campaign archive require confirmation.
- Copy-all and import completion produce a visible, specific result.
- All dialogs trap focus and restore it on close.
- Light/dark themes meet contrast requirements on text and status chips.
