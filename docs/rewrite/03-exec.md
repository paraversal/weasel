# 03-exec — Execution plan for the data model

Turns [03-data-model.md](03-data-model.md) into ordered milestones. Dense by design: every
line is an instruction or a decision. Read fully before touching the repo; work milestones
in order; each ends green and in one commit.

- **Written:** 2026-11-11, after 02-exec landed (`78b411f` on `rewrite/02-architecture`).
- **Effort:** ≈7 ideal days. Covers 06 phase 2 ("data core") **plus** everything below the
  IPC line that phases 4–5 need: import parsing, reconciliation planner, apply/discard.
  After this plan, 04-exec is only commands/DTOs/events/mock; 08-exec only screens.
- **Read first:** 03 (all; DDL is the source), 02-exec §6–§11 (what exists, patterns),
  04 §Conventions/§Shared DTOs/§Command surface (what services must support), 05 §Backend
  extension points, 01 §Behaviours worth preserving/§Defects. Ignore everything legacy
  (ADR 0006).

## 1. Scope

**In:** `migrations/0002_domain.sql`; domain types, validation, ports and services for
contacts, campaigns, sends, imports, audit (`weasel-core`); sqlx repositories with
in-transaction audit (`weasel-store`); source readers (csv/xlsx/xls/ods), header detection,
column mapping, row normalization, reconciliation planner, `ImportService`
(`weasel-import`); testkit builders + `seed-demo` binary; adapter composition +
additive `app_health` fields; backup restore helper.

**Out (owner):** domain IPC commands, DTOs, `DomainEvent` variants, mock handlers
(04-exec) · screens (08-exec) · import undo, configurable identity strategy, conflict UI
(phase 5) · FTS5, virtualization, perf budgets (phase 6) · `app_backup_create` command
(04-exec; `Store::backup_now` exists) · direct sending, tags, templates (phase 8).
Do not add placeholder modules for out-of-scope items; leave `TODO(04-exec)` only where a
hook is needed.

## 2. Ask the owner before M0 (defaults in bold; do not guess past them)

1. **Branch:** `rewrite/02-architecture` is not merged. **Default: branch
   `rewrite/03-data-model` from it (stacked), one commit per milestone, no pushes.**
2. **Identity:** do real rosters carry a stable student/matriculation number? **Default:
   no → identity key = names only (`name-v1`).** If yes, *before M1*: add nullable
   `contacts.external_id TEXT` + partial unique index on live rows, an `externalId`
   mapping field, and an `ExternalIdIdentity` strategy preferred over names. Must be
   decided before 0002 is written (migrations are immutable once applied to a dev DB).
3. **Clipboard separator** for prepared sends. Legacy joins with a space. **Default: keep
   `" "`** behind one constant (`send::RECIPIENT_SEPARATOR`); a setting comes later if
   wanted.
4. **Sheet layout:** an anonymized header row + 2 fake rows of a real roster. **Default:**
   synthesize fixtures matching legacy (`header=1` i.e. header on row 2, one footer row,
   columns `VORNAME`/`NACHNAME`/`EMAIL`).
5. **CSV encoding:** **Default UTF-8 only** (BOM stripped); non-UTF-8 → `IMPORT` error
   "Save the file as CSV UTF-8". If the owner exports CSV from German Excel, add
   `encoding_rs` Windows-1252 fallback in M5.
6. Untracked `csp.ipynb`/`test.ipynb` at the root may contain real data: **never stage
   them** (`git add` paths explicitly).

## 3. Versions (verified 2026-11-11; take latest patch within these)

`calamine 0.38` · `csv 1.4` · `sha2 0.11` · `unicode-normalization 0.1` (core, NFC) ·
`proptest 1.12` (dev) · `rust_xlsxwriter 0.102` (dev, fixture generation) · existing
`sqlx 0.9.0` (features already: runtime-tokio, sqlite, migrate, macros, json — `FromRow`
derive is in `macros`). Add all to `[workspace.dependencies]`; crates use
`workspace = true`.

## 4. Facts that will bite

1. **Dev-dependency cycle = duplicate crate types.** `weasel-testkit` depends on
   core/store/import; those crates dev-depend on testkit. Using testkit inside a
   `#[cfg(test)] mod` of core/store/import compiles a second copy of the crate →
   "expected `weasel_core::X`, found `weasel_core::X`". **Rule:** testkit only in
   `crates/*/tests/*.rs` integration tests; `#[cfg(test)]` modules use in-file fakes or
   pure functions.
2. **`pool.begin()` is `BEGIN DEFERRED`.** Read-then-write (prepare send, apply import,
   versioned update) can race or hit `SQLITE_BUSY` on lock upgrade despite
   `busy_timeout`. Add `Store::begin_write()` = `BEGIN IMMEDIATE` (sqlx 0.9:
   `pool.begin_with("BEGIN IMMEDIATE")`; verify the name, else
   `pool.acquire()` + `conn.begin_with(..)` returning an owned tx). **Every write path uses
   it**, including the existing `SqliteSettings::save` (switch it in M1).
3. **`sqlx::migrate!` is compile-time.** `weasel-store` has no `build.rs`, so adding
   `0002_domain.sql` may not trigger a rebuild. Add `crates/weasel-store/build.rs`:
   `println!("cargo:rerun-if-changed=../../migrations");`.
4. **SQLite `LIKE` and `lower()` are ASCII-only case-insensitive.** Normalize in Rust
   (stored `email_normalized`); search names with `LIKE` and accept that `Ü`≠`ü` in v1
   (document; FTS5 phase 6). Escape `\ % _` and use `ESCAPE '\'`.
5. **Restoring a WAL database:** close the pool first, delete `<db>-wal`/`<db>-shm`, then
   swap the file (copy to `<db>.restore-tmp`, fsync, rename). A stale `-wal` over a
   restored file corrupts it.
6. `FakeClipboard::fail_next` returns `CoreError::InvalidState`; adapters map clipboard
   failure to `Io` (retryable). Fix testkit to return `CoreError::Io` (M4).
7. Dev DBs created by 02-exec are at schema v1. First `pnpm dev` after M1 backs up and
   migrates — that is the real-upgrade manual check (M8).
8. `desktop/src/lib/ipc/mock.ts` mirrors `app_health`; changing the DTO without the mock
   breaks `mock.test.ts` (M8).
9. `plan_json` can be MBs at 10k rows: list queries must name columns, never `SELECT *`
   from `import_batches`.
10. calamine returns numbers as `f64`: integral floats → no `.0` (`12345.0` → `"12345"`);
    `DateTime` → ISO date; `Empty`/`Error` → `""`.
11. Legacy "not sent" included removed people (defect 2). New audience = **active
    only**. Deliberate; don't "fix" toward parity.
12. `tools/check-layers.sh` checks direct manifest deps only. Add `tokio` to the
    `weasel-import` deny list (it stays runtime-agnostic: sync parsing, async only via
    ports).

## 5. Target additions

```text
migrations/0002_domain.sql
crates/weasel-core/src/{email.rs,identity.rs,query.rs,contact.rs,campaign.rs,send.rs,import.rs,audit.rs}  (+ ports.rs, error.rs edits)
crates/weasel-store/build.rs
crates/weasel-store/src/{tx.rs,codec.rs,audit.rs,contacts.rs,campaigns.rs,sends.rs,imports.rs,restore.rs}
crates/weasel-store/tests/{schema.rs,contacts.rs,campaigns.rs,sends.rs,restore.rs,upgrade.rs}
crates/weasel-import/src/{source.rs,read/{mod.rs,csv.rs,sheet.rs},header.rs,mapping.rs,normalize.rs,reconcile.rs,service.rs}
crates/weasel-import/tests/{read.rs,reconcile_table.rs,reconcile_props.rs,apply.rs,fixtures/*.csv}
crates/weasel-testkit/src/{builders.rs,assert.rs,services.rs,bin/seed_demo.rs}
desktop/src-tauri/src/{state.rs,lib.rs,commands/app.rs} edits; bindings.ts†; ipc/mock.ts; dashboard AppStatusWidget
```

## 6. Milestones

### M0 — Branch and deps (≈0.1 d)
Resolve §2. Branch. Add §3 deps to workspace. Add `tokio` to weasel-import deny list in
`check-layers.sh`. Commit with M1.

### M1 — Schema and store plumbing (≈0.75 d)

`migrations/0002_domain.sql` = 03 §Initial schema DDL for `import_batches`, `contacts`,
`contact_email_history`, `campaigns`, `send_batches`, `send_records`, `audit_events`
(**not** `settings`/`app_meta`, already in 0001), **verbatim except**:
- `import_batches.source_kind` CHECK: `('xlsx','xls','ods','csv','manual')` (no `legacy`).
- `import_batches` add `sheet_name TEXT`, `invalid_count INTEGER NOT NULL DEFAULT 0`.
  Keep status `failed` in the CHECK (reserved, unused in v1).
- `CREATE UNIQUE INDEX ux_email_history_open ON contact_email_history(contact_id) WHERE valid_to IS NULL;` (invariant 2 enforced).
- Trigger `send_records_snapshot_immutable`: `BEFORE UPDATE OF batch_id, email, email_normalized, first_name, last_name ON send_records` → `RAISE(ABORT, 'send record snapshot is immutable')` (invariant 3; `contact_id` SET NULL and `status` stay updatable).
- Triggers `audit_events_no_update` / `audit_events_no_delete` → `RAISE(ABORT, 'audit log is append-only')`.
- Header comment: "Never edit after it has shipped."
Plus §2.2 columns if the owner said yes.

Store plumbing:
- `build.rs` (§4.3). `tx.rs`: `Store::begin_write() -> Result<Transaction<'static, Sqlite>, StoreError>`; switch `SqliteSettings::save`.
- `codec.rs`: `fn decode_enum<T: FromStr>(col: &str, s: String) -> CoreResult<T>` (unknown → `Internal`), helpers for JSON columns. Rows read via `#[derive(sqlx::FromRow)]` private row structs → `From`/`TryFrom` into core types. Runtime `query`/`query_as` only (no `query!`, no `DATABASE_URL`).
- `error.rs`: map unique violations by constraint text: `contacts.email_normalized` → `Conflict("Another contact already uses this email")`; others → `Conflict(<generic>)`. FK violation / trigger `RAISE` → `Internal` (bugs, not user errors).
- `restore.rs`: `pub fn restore_backup(backup: &Path, db_path: &Path) -> Result<(), StoreError>` per §4.5; caller guarantees the store is closed. `Store::close(self)` → `pool.close().await`.
- Testkit `assert.rs`: `assert_fk_clean(&Store)` (`PRAGMA foreign_key_check` empty) and `assert_integrity(&Store)` (`PRAGMA integrity_check` = ok). Every write test below ends with `assert_fk_clean`.

Tests (`weasel-store/tests/schema.rs`, `upgrade.rs`, `restore.rs`):
- fresh `TempDb` → schema v2; snapshot sorted list of table/index/trigger names from `sqlite_master` (change requires updating the snapshot — intentional friction).
- constraints: live-email uniqueness (active+removed dup fails; archived dup ok); second open history row fails; snapshot trigger fires; audit update/delete abort; each enum CHECK rejects a bogus value.
- **Rust↔SQL enum drift:** for every variant of every core status/kind enum, inserting `as_str()` succeeds (written in M2/M3 once the enums exist; list it here so it isn't forgotten).
- upgrade: copy `migrations/0001_app.sql` alone into a temp migrations dir → `Migrator::new(dir)` → `open_with` (v1) → `open` (real) → backup file exists and contains v1 only, `app_meta.last_backup_path` set, domain tables present.
- restore: create data → `backup_now` → mutate → close → `restore_backup` → reopen → data equals pre-mutation state; integrity ok.

Done when: `cargo test --workspace`, clippy `-D warnings`, `pnpm check:layers` green.
Commit: `feat(store): domain schema v2, immediate write transactions, restore helper`

### M2 — Core domain types, validation, ports (≈0.75 d)

Pure code + unit tests (no store). One module per file; add to `lib.rs` above `// @gen:modules`.

- `error.rs`: add `CoreError::Import { row: Option<u32>, reason: String }`; adapter maps → `ErrorCode::Import` (retryable false, details `{row}`), extend the M5 mapping test; drop `#[allow(dead_code)]` on `Import`. Regenerate bindings only if the TS changes (it shouldn't).
- `email.rs`: `EmailAddress { display, normalized }`; `parse(raw)`: trim; reject empty, whitespace inside, ≠1 `@`, empty local/domain, domain without `.` or with leading/trailing `.`, length > 254 → `Validation{field:"email"}`. `normalized = display.to_lowercase()` (whole address, per 03 Conventions). Not RFC 5322; documented.
- `identity.rs`: `clean_name(raw)` = trim, collapse internal whitespace runs to one space, NFC. `trait IdentityStrategy: Send+Sync { fn id(&self) -> &'static str; fn key(&self, first: &str, last: &str) -> Option<String>; }`. `NameIdentity` (`"name-v1"`): `lower(clean(first)) + '\u{1f}' + lower(clean(last))`, `None` if either empty. **Plain normalized string, not a hash** (debuggable, collision-free, no new PII). Store `app_meta.identity_strategy = "name-v1"` on first domain write (M3); mismatch at boot → log warning only (recompute is phase 5).
- `query.rs`: `PageRequest { limit, offset }` → `normalized()`: limit default 100, clamp 1..=500, offset ≥ 0. `Page<T> { items, total, offset, limit }` + `map`. `like_pattern(term) -> Option<String>`: trim, empty → None, lowercase, escape `\ % _`, wrap `%…%`.
- Enums (all `as_str()` + `FromStr` + serde `camelCase`/`snake_case` matching the SQL text; **no sqlx/specta derives in core**): `ContactStatus{Active,Removed,Archived}`, `ContactSource{Import,Manual,Api}`, `EmailChangeReason{Import,Manual,Merge,Correction}`, `CampaignStatus{Draft,Active,Archived}`, `SendRecordStatus{Created,Copied,Sent,Failed,Skipped}`, `ImportStatus{Draft,Applied,Failed,Discarded}`, `ImportScope{Roster,Delta}`, `SourceKind{Xlsx,Xls,Ods,Csv,Manual}`.
- Limits (consts, validated in services, `Validation` with the field name): names 1..=200 chars after `clean_name`; notes ≤ 10 000; campaign name 1..=200, description ≤ 10 000, subject ≤ 998, body ≤ 100 000; batch note ≤ 2 000.
- `contact.rs`: `Contact {id, first_name, last_name, email, email_normalized, identity_key, status, source, import_batch_id, date_added, updated_at, version, notes, metadata: Value}`; `NewContact {first_name, last_name, email, notes}`; `ContactPatch {version, first_name?, last_name?, email?, notes?}`; `ContactSort{LastNameAsc,LastNameDesc,AddedDesc,EmailAsc}` (store maps each to a fixed ORDER BY, always `…, id`); `ContactQuery {search, statuses: Vec (empty → [Active, Removed]; invariant 8), import_batch_id, sort, page}`; `EmailHistoryEntry {email, valid_from, valid_to, reason}`; `CampaignReceipt {campaign_id, campaign_name, batch_id, sent_at}`; `ContactDetail {contact, email_history, campaigns}`; `ContactStats {active, removed, archived, total, by_batch: Vec<BatchCount{batch_id, source_name, count}>}`; `fn can_transition(ContactStatus, ContactStatus) -> bool` (any → any other).
- `campaign.rs`: `Campaign {id, name, description, subject, body, status, created_at, updated_at, version}`; `NewCampaign`, `CampaignPatch {version, …?}`; transitions draft→active|archived, active→archived, archived→active; `CampaignQuery {status?, search, page}`; `CampaignStats {sent, unsent, removed, total, last_batch_at}` — **definitions:** `sent` = send records for the campaign (incl. deleted contacts), `unsent` = active contacts with no record, `removed` = removed contacts with no record, `total = sent + unsent`; `CampaignSummary {campaign, stats}`; `AudienceFilter{Sent,Unsent,All}`; `AudienceMember {contact_id: Option, first_name, last_name, email, contact_status: Option, batch_id: Option, sent_at: Option}` (sent rows come from snapshots).
- `send.rs`: `SendBatch {id, campaign_id, campaign_name, created_at, recipient_count, note, copied_at}`, `SendRecord {id, batch_id, contact_id?, email, first_name, last_name, status, created_at}`, `SendBatchQuery {campaign_id?, search (note, campaign name, recipient name/email), page}`, `PreparedSend {batch, emails: Vec<String>}`, `RECIPIENT_SEPARATOR`, `format_recipients(&[String]) -> String`.
- `import.rs` (types only; logic in weasel-import): `ImportBatch {id, source_kind, source_name, source_path?, file_hash?, sheet_name?, scope, status, created_at, applied_at?, summary: ImportSummary, error?}` (no plan); `ImportSummary {row_count, added, updated, removed, conflicts, unchanged, invalid}`; `NormalizedRow {row: u32 /*1-based sheet row*/, first_name, last_name, email: EmailAddress, identity_key: Option}`; `RowError {row, field, reason, identity_key: Option}`; `ContactRef {id, version, first_name, last_name, email, email_normalized, identity_key, status, source}`; `ReconcileSnapshot {contacts: Vec<ContactRef> /*active+removed only*/, history: Vec<(String /*email_normalized*/, i64 /*contact_id*/)>}`; `PlanAction` (serde tag `kind`): `Add`, `UpdateDetails{target}`, `UpdateEmail{target}`, `Restore{target}`, `Noop{contact_id}`, `Conflict{index}` with `target: ContactRef` (carries `version` for staleness); `PlannedRow {input: NormalizedRow, action}`; `PlannedRemoval {contact: ContactRef}`; `ConflictKind{DuplicateEmailInSource, AmbiguousHistory, AmbiguousIdentity, ManualContact, ContactMatchedTwice}`; `Conflict {rows: Vec<u32>, kind, candidates: Vec<ContactRef>}`; `PlanWarning{LargeRemoval{count, active_import_contacts}, SameFileAsBatch{batch_id}, IgnoredTrailingRows{count}}`; `ImportPlan {version: u32 /*PLAN_VERSION=1*/, scope, identity_strategy, mapping: ColumnMapping, rows, removals, conflicts, errors, warnings, summary}`; `ColumnMapping {first_name: u32, last_name: u32, email: u32}` (column indexes); `ConflictResolution {row, action: LinkContact{contact_id} | CreateNew | Skip}`; `ContactOp`: `Create{row, first_name, last_name, email, identity_key}` | `Modify{row: Option, contact_id, expected_version, first_name?, last_name?, email?: EmailAddress, status?: ContactStatus, identity_key?}`; `ImportPhase{Reading, Matching, Applying}`.
- `audit.rs`: `pub mod actions` consts — `contact.create|update|email_change|status|delete|import_create|import_update`, `campaign.create|update|status|delete`, `send_batch.create|recopy|note`, `import.preview|apply|discard`; `AuditEvent {id, at, actor, action, entity_type, entity_id, before, after, metadata}`; `AuditRecord` (write-side, no id/at).
- `ports.rs` additions (all `#[async_trait]`, `Send+Sync`, `CoreResult`; write methods take `now_ms: i64` like settings; **each write writes its own audit rows in the same tx**):
  - `ContactRepository`: `list(&ContactQuery) -> Page<Contact>`, `get(id)`, `detail(id)`, `stats()`, `count_active()`, `create(NewContactRecord{…, source, identity_key}, now)`, `update(id, version, ContactChanges, EmailChangeReason, now)`, `set_status(id, version, status, now)`, `delete(id, version, now)`.
  - `CampaignRepository`: `list`, `get`, `create`, `update`, `set_status`, `delete`, `stats(id)`, `audience(id, filter, search, page)`.
  - `SendRepository`: `begin_batch(campaign_id, now) -> Box<dyn PendingSendBatch>`, `list`, `get`, `recipients(id, search, page)`, `emails(id) -> Vec<String>` (record-id order), `mark_recopied(id, now)`, `set_note(id, note, now)`. `trait PendingSendBatch: Send { fn batch(&self) -> &SendBatch; fn emails(&self) -> &[String]; async fn commit(self: Box<Self>) -> CoreResult<()>; }` — owns the `Transaction<'static>`; **drop = rollback**.
  - `ImportRepository`: `snapshot() -> ReconcileSnapshot`, `create_draft(NewImportBatch{source meta, scope, plan, summary}, now) -> ImportBatch`, `get(id)`, `plan(id) -> ImportPlan`, `list(status?, page)`, `members(id, page) -> Page<Contact>` (contacts whose `import_batch_id` = id), `last_applied_with_hash(hash) -> Option<i64>`, `apply(id, Vec<ContactOp>, ImportSummary, now) -> ImportBatch`, `discard(id, now)`, `count_drafts()`.
  - `AuditRepository`: `for_entity(entity_type, entity_id, page)`, `recent(page)`.
  - `ProgressSink { fn report(&self, phase: ImportPhase, processed: u64, total: u64); }` + `NoProgress`.
- Unit tests: email accept/reject table; `clean_name`/identity key (whitespace, case, NFC vs NFD `Mu\u{308}ller`); `like_pattern` escaping; page clamping; transition tables; serde round-trip of `ImportPlan` (shape is persisted — a change requires bumping `PLAN_VERSION`).

Commit: `feat(core): domain types, validation, identity key, repository ports`

### M3 — Contacts and audit (≈1 d)

`weasel-store/src/audit.rs`: `pub(crate) async fn record(tx, &AuditRecord, now)`; `SqliteAudit` implements `AuditRepository`. Before/after JSON = serialized domain struct (whole `Contact`), metadata e.g. `{"importBatchId", "row", "reason"}`.

`contacts.rs` (`SqliteContacts`), all writes via `begin_write`:
- create: insert contact (`date_added = updated_at = now`, version 1) + open history row (reason from source: import→`import`, manual→`manual`) + audit; on first-ever domain write upsert `app_meta.identity_strategy`.
- update: `UPDATE … SET …, version = version + 1, updated_at = ? WHERE id = ? AND version = ?`; 0 rows → `SELECT 1` → `NotFound{entity:"contact"}` else `Conflict("This contact was changed elsewhere. Reload and try again.")`. If email changed: close open history row (`valid_to = now`), insert new open row with reason; audit `contact.email_change` (else `contact.update`). Recompute `identity_key` when names change (service supplies it).
- set_status: versioned; archived→active/removed may hit live-email unique index → `Conflict`.
- delete: refuse with `InvalidState("This person has send history; archive instead")` if any `send_records.contact_id = id`; else delete (history cascades) + audit with `before`.
- list: one parameterized SQL built from fixed fragments (status `IN` via repeated `?`, search `(lower(first_name) LIKE ?1 ESCAPE '\' OR lower(last_name) LIKE ?1 … OR email_normalized LIKE ?1)`, batch filter), `COUNT(*)` with the same WHERE for `total`. Never interpolate user text.
- detail: history newest first; campaigns via `send_records ⋈ send_batches ⋈ campaigns` newest first.
- stats: `GROUP BY status`; by_batch joins `import_batches.source_name`.

`weasel-core::contact::ContactService { repo, clock, identity }`: `create_manual(NewContact)` (validate, clean names, parse email, key, source manual), `update(id, ContactPatch)` (no-op patch → return current without write), `set_status`, `delete`, `get`, `detail`, `list`, `stats`, `count_active`. Services own validation; repos trust input.

Tests (`weasel-store/tests/contacts.rs`, TempDb + real services): create/get; duplicate live email → `Conflict`; archived frees the email; un-archive into a taken email → `Conflict`; stale version → `Conflict`; missing id → `NotFound`; email change: exactly one open history row, previous closed at `now`, audit `contact.email_change`; delete with send record → `InvalidState`, without → history gone, audit kept; list: default excludes archived, `%`/`_` searched literally, every sort stable with ties, limit 1000 → 500, total correct; stats; enum-drift test (§M1) for contact enums; `assert_fk_clean` after each.

Commit: `feat(contacts): repository, service, audit trail`

### M4 — Campaigns and sends (≈1 d)

`campaigns.rs`: CRUD versioned like contacts; `delete` only if no `send_batches` (else `InvalidState("Campaign has send history; archive it instead")`; invariant 7); stats/audience per M2 definitions. Unsent query: `contacts c WHERE c.status='active' AND NOT EXISTS (SELECT 1 FROM send_records r JOIN send_batches b ON b.id = r.batch_id WHERE b.campaign_id = ? AND r.contact_id = c.id)`; order `last_name, first_name, id`. `All` = sent snapshots `UNION ALL` unsent active, paginated over the union.

`sends.rs` (`SqliteSends`): `begin_batch`: `begin_write` → campaign exists (`NotFound`) and `status = active` (`InvalidState("Only active campaigns can be sent")`) → select unsent recipients → none → `InvalidState("Everyone in the audience has already received this campaign")` → insert `send_batches` (`recipient_count`, `copied_at = now`) + one `send_records` row per recipient (snapshot names/emails, `status = 'copied'`) + audit `send_batch.create` (metadata `{campaignId, recipientCount}`, no email list) → return pending (uncommitted). `mark_recopied`: update `copied_at` + audit. `set_note`: validate + audit. `list` search matches note, campaign name, or `EXISTS` recipient match.

`weasel-core::send::SendService { sends, clipboard, clock }`:
- `prepare_send(campaign_id) -> PreparedSend`: `pending = begin_batch` → `clipboard.write_text(format_recipients(pending.emails()))` → on `Err` drop pending (rollback) and return the error → `pending.commit()` → return. Clipboard **before** commit (02 §Request flow): a clipboard failure records nothing; a commit failure after a successful copy returns the error with nothing recorded (retry is safe). The write lock is held across the clipboard call by design (milliseconds).
- `recopy(batch_id)`: `emails(id)` → clipboard → `mark_recopied`. Returns same shape.
- `set_note`, `list`, `get`, `recipients`.
`CampaignService { campaigns, clock }`: validation + transitions + reads.

Testkit: `FakeClipboard` failure → `CoreError::Io` (§4.6).

Tests (`weasel-store/tests/campaigns.rs`, `sends.rs`): transitions table incl. rejected ones; delete with batches refused; stats definitions on a mixed fixture (active/removed/archived, deleted contact with record); audience pagination/search for each filter; prepare_send: clipboard text == `format_recipients(returned emails)`, records == recipients, second call → `InvalidState`, removed/archived excluded, contact added afterwards is unsent; **clipboard failure → zero batches/records, error `Io`**; archived/draft campaign refused; **recopy after editing a recipient's email returns the original snapshot**; snapshot trigger blocks a direct UPDATE; contact delete sets `contact_id NULL` but audience `Sent` still lists the snapshot; **concurrency:** two `prepare_send` on one campaign via `tokio::join!` → exactly one `Ok`, one `InvalidState`, records == audience size (proves `BEGIN IMMEDIATE`).

Commit: `feat(sends): campaigns, send batches with clipboard-before-commit`

### M5 — Import sources, headers, mapping, normalization (≈1 d)

`weasel-import` deps: core, calamine, csv, sha2, serde, serde_json, async-trait, tracing. Dev: proptest, rust_xlsxwriter, tempfile, tokio, weasel-store, weasel-testkit. Update the crate doc (replaces "intentionally empty").

- `source.rs`: `ReadOptions { sheet: Option<String>, max_bytes = 20 MiB, max_rows = 100_000 }`; `ParsedSource { kind, name /*file name only*/, path, file_hash /*sha256 hex of bytes*/, sheet_name, sheets: Vec<String>, header_row: u32, headers: Vec<String>, rows: Vec<RawRow{row: u32, cells: Vec<String>}>, ignored_trailing: u32 }`; `pub fn read_source(path, &ReadOptions) -> CoreResult<ParsedSource>` — **sync**; caller (04-exec) runs it in `spawn_blocking` after `weasel_platform::fs::validate_source_path`. Kind by extension (`xlsx|xlsm→Xlsx`, `xls`, `ods`, `csv`); unknown → `Import`. Size/row limits → `Import` with numbers in the message.
- `trait SourceReader { fn kinds(&self) -> &[SourceKind]; fn read(&self, bytes: &[u8], opts) -> CoreResult<RawTable>; }` + static registry `[CsvReader, SheetReader]` — the 05 `ContactImporter` extension point, kept in this crate (deviation 7).
- `read/csv.rs`: strip UTF-8 BOM; non-UTF-8 → `Import` (§2.5); delimiter sniff over `, ; \t` by consistent count across the first 5 non-empty lines (`;` wins ties — German Excel); `flexible(true)`; trim cells.
- `read/sheet.rs`: `calamine::open_workbook_auto_from_rs(Cursor)`; first sheet unless `sheet` given (unknown → `Import` listing sheets); cell formatting per §4.10; trailing empty columns dropped.
- `header.rs`: synonyms (case/space/`-`/`_`-insensitive, umlaut-folded): first = `vorname, first name, firstname, given name, forename`; last = `nachname, last name, lastname, surname, family name, name`; email = `email, e-mail, mail, email address, e-mail-adresse`. Header row = first of the first 10 rows matching ≥ 2 fields; none → row 1 and `suggested_mapping` empty. Trailing rows after the last row with a non-empty email-column cell are ignored → `ignored_trailing` (handles legacy footer).
- `mapping.rs`: `suggest(headers) -> SuggestedMapping {first_name?, last_name?, email?}`; `resolve(explicit: Option<ColumnMapping>, suggested, width) -> CoreResult<ColumnMapping>` (missing field or out-of-range index → `Validation{field:"mapping"}`; same column twice → `Validation`).
- `normalize.rs`: `normalize(&ParsedSource, &ColumnMapping, &dyn IdentityStrategy) -> (Vec<NormalizedRow>, Vec<RowError>)`; fully blank mapped cells → skipped silently; otherwise names via `clean_name` (empty → error `firstName`/`lastName`), email via `EmailAddress::parse` (error carries `identity_key` if names are valid — used for removal protection).

Fixtures: `tests/fixtures/*.csv` committed (synthetic names only, `@example.edu`): comma, semicolon+BOM, tab, blank lines, footer row, duplicate email, invalid email, whitespace/case variants. xlsx/ods built at test time with `rust_xlsxwriter` into a tempdir (legacy layout: title row, header row 2, footer). xls/ods: one tiny committed binary each only if you can produce it locally without real data; otherwise test the kind-dispatch path only and note it in §10.

Tests (`tests/read.rs`): each fixture → expected headers/rows/ignored_trailing; header detection on row 2; numeric cells; size/row limits; unknown extension; sheet selection; mapping suggestion incl. `E-Mail-Adresse`, `Vorname`; normalization errors.

Commit: `feat(import): csv/xlsx/ods readers, header detection, column mapping`

### M6 — Reconciliation planner (≈1 d)

`reconcile.rs`, pure: `pub fn plan(rows, errors, &ReconcileSnapshot, scope, mapping, strategy_id) -> CoreResult<ImportPlan>` (+ `progress` every 500 rows, phase `Matching`). Algorithm, in order:

1. **Source duplicates:** group valid rows by `email.normalized`; groups > 1 → one `Conflict{DuplicateEmailInSource, rows, candidates: [live contact with that email if any]}`; those rows leave matching.
2. **Match** each remaining row: (a) live contact (active/removed) with current `email_normalized` → that one; (b) else history entries for the email → distinct non-archived contacts: 1 → match, >1 → `AmbiguousHistory`; (c) else identity key → contacts with that key: 1 → match, >1 → `AmbiguousIdentity`; (d) else `Add`. Archived contacts are not in the snapshot, so a returning archived person becomes a new contact (03 Invariant 8 + partial unique index).
3. **Manual guard:** a match on a `source != import` contact (any path) → `Conflict{ManualContact, candidates:[it]}` (03 rule 4).
4. **Double match:** a contact matched by > 1 row → one `Conflict{ContactMatchedTwice}` over those rows.
5. **Action for (row, contact)** — `pub fn action_for(row, &ContactRef) -> PlanAction`, reused by conflict resolution: contact `removed` → `Restore` (folds in name/email changes); email differs → `UpdateEmail` (folds in name changes); names differ (exact compare after `clean_name`) → `UpdateDetails`; else `Noop`.
6. **Removals (roster only):** contacts with `status = active`, `source = import`, not matched, not a candidate in any conflict, and whose `identity_key` ∉ keys of `RowError`s (invalid rows protect their person) → `PlannedRemoval`. Delta → none. Already-removed contacts absent from the sheet: untouched, not counted.
7. **Safety:** roster with 0 valid rows → `Validation{field:"source", "No valid rows; a roster import would remove everyone"}`. Removals > 50 % of active import contacts (and > 5) → `PlanWarning::LargeRemoval`. `ignored_trailing > 0` → warning.
8. Summary counts from actions (`updated = update_details + update_email + restore`, `conflicts = conflict count`, `invalid = errors`).

`pub fn resolve(plan, &[ConflictResolution]) -> CoreResult<Vec<ContactOp>>`: every conflict row needs exactly one resolution (missing → `InvalidState("{n} rows need a decision")`); unknown row → `Validation{field:"resolutions"}`; `LinkContact` only to a conflict candidate (→ `action_for`), `CreateNew` invalid when a live contact has the email or twice for one email, `Skip` always ok; after resolution no contact may be targeted twice. Map actions → ops: `Add→Create`, `UpdateDetails/UpdateEmail/Restore→Modify{expected_version: target.version, changed fields only, status: Some(Active) for Restore}`, removals → `Modify{status: Removed}`, `Noop`/`Skip` → nothing.

Tests:
- `tests/reconcile_table.rs`, one named case each: new → add · same → noop · names changed → update_details · historical email → update_email · new email + key → update_email · key ×2 → AmbiguousIdentity · history ×2 → AmbiguousHistory · email of removed → restore · key of removed + new email → restore with email · roster absent import → mark_removed · roster absent manual → none · delta absent → none · absent removed → none · dup email rows → DuplicateEmailInSource · email of manual → ManualContact · email-row + key-row same contact → ContactMatchedTwice · invalid row with matching names protects removal · archived same email → add · conflict candidates not removed · empty roster refused · large-removal warning · each resolution kind incl. rejected ones.
- `tests/reconcile_props.rs` (proptest over small random snapshots/sheets, ≤ 30 rows, tiny name/email alphabets to force collisions): every input row appears exactly once across rows/conflicts/errors; no contact targeted twice by ops; applying ops to an in-memory model never yields duplicate live emails; delta ⇒ no removals; manual contacts never removed/modified without `LinkContact`; summary == action counts; **re-planning the same sheet against the applied model ⇒ only noops** (idempotence; conflicts resolved by `Skip`).

Commit: `feat(import): reconciliation planner with conflict resolution`

### M7 — Import persistence and ImportService (≈0.75 d)

`weasel-store/src/imports.rs` (`SqliteImports`):
- `snapshot`: active+removed contacts (id, version, names, emails, key, status, source) + all history `(email_normalized, contact_id)` for them. Two queries, no N+1.
- `create_draft`: insert batch (`status = draft`, counts from summary, `plan_json`) + audit `import.preview` (metadata: counts, scope, file hash; no rows).
- `apply`: `begin_write` → `UPDATE import_batches SET status='applied' … WHERE id=? AND status='draft'` (0 rows → `InvalidState("Import was already applied or discarded")`) → ops in order through the **same private helpers as `SqliteContacts`** (create/versioned modify/history/audit — no duplicated email-history logic; audit actions `contact.import_create`/`contact.import_update`, metadata `{importBatchId, row}`); `Create` sets `source='import'`, `import_batch_id`; email change reason `import`; any versioned miss → `Conflict("Contacts changed since the preview. Preview the file again.")`, any unique violation → `Conflict` — **whole tx rolls back** → final counts + `applied_at` + audit `import.apply`.
- `discard`: draft only; `status='discarded'`, **`plan_json = NULL`** (rows that were never imported are PII we don't need) + audit. Applied batches keep `plan_json` (phase-5 undo needs it).

`weasel-import/src/service.rs` — `ImportService { imports: Arc<dyn ImportRepository>, clock, identity: Arc<dyn IdentityStrategy> }`:
- `preview(source: ParsedSource, PreviewOptions { mapping: Option<ColumnMapping>, scope }, &dyn ProgressSink) -> ImportPreview { batch, headers, header_row, suggested_mapping, mapping, summary, sample: first 100 PlannedRows, conflicts: all, errors: all, warnings }`: resolve mapping → normalize → snapshot → `plan` → `SameFileAsBatch` warning via `last_applied_with_hash` → `create_draft`.
- `apply(batch_id, Vec<ConflictResolution>) -> ImportBatch`: `get` (draft else `InvalidState`) → `plan` (`version != PLAN_VERSION` → `InvalidState("Preview the file again")`) → `resolve` → summary from final ops → `imports.apply`.
- `discard`, `list`, `get`, `members`, `plan_rows(batch_id, page)` (paginates the stored plan for resuming review).

Tests (`weasel-import/tests/apply.rs`, TempDb + real services, xlsx fixture): end-to-end add; **re-preview same file → added = removed = updated = 0** (+ `SameFileAsBatch`); changed email keeps contact id, history 2 rows, 1 open; roster removal → `removed`, its send records intact; reappearing → `restore`; unresolved conflicts → `InvalidState`, no writes, still draft; **stale:** preview → edit a contact via `ContactService` → apply → `Conflict`, zero changes; **forced mid-tx failure:** preview adds A,B,C → manually create C's email → apply → `Conflict`, A and B absent (full rollback); double apply → `InvalidState`; discard → contacts untouched, `plan_json IS NULL`; audit: one row per op + `import.apply`; manual contact never removed by a roster; `assert_fk_clean` everywhere.

Commit: `feat(import): draft/apply/discard import batches`

### M8 — Testkit, demo data, adapter composition (≈0.5 d)

- Testkit: `builders.rs` (`ContactSeed`, `CampaignSeed`, `parsed_source(rows)` building a `ParsedSource` in memory), `services.rs` (`TestServices::new(&TempDb)` → all services + `FixedClock` + `FakeClipboard`), deps += weasel-import, tokio.
- `src/bin/seed_demo.rs` (`[[bin]] name = "seed-demo"`): `seed-demo --data-dir <dir> [--contacts 500] [--seed 42] [--force]`; **requires** `--data-dir` (never defaults to real app data), refuses an existing DB without `--force`; opens via `weasel_store::open`, writes **only through services** (proves them): 3 roster imports with churn (email changes, removals, a returnee), 5 manual contacts, 4 campaigns (one archived), 3 send batches via `FakeClipboard`; deterministic xorshift RNG (no `rand`), synthetic German-ish names, `@example.edu`. Root script `"seed:demo": "cargo run -q -p weasel-testkit --bin seed-demo --"`; README: `pnpm seed:demo --data-dir .dev-data/demo && WEASEL_DATA_DIR=.dev-data/demo pnpm dev`. `--contacts 50000` must finish < 60 s release (phase 6 uses it).
- Perf smoke (not a budget): `#[ignore]` test `perf_contacts_list_50k` in `weasel-store/tests/` printing first-page/search/audience timings; run with `cargo test --release -- --ignored perf_`; record numbers in §10.
- Adapter: add `weasel-import` dep; `Services` gains `contacts`, `campaigns`, `sends`, `imports`, `audit` (built in `open_services` from one `Store` clone each; `NameIdentity`; `#[allow(dead_code)]` with `// TODO(04-exec)` on fields no command uses yet). `AppHealthDto` += `contact_count: Option<i64>`, `pending_imports: Option<i64>` (None if boot failed or the query errors — health never fails). No `API_VERSION` bump (additive, 04 §Compatibility). `pnpm bindings`; update `mock.ts` health handler + its test; `AppStatusWidget` shows both counts.
- Manual check with `pnpm dev` on the existing v1 dev DB: backup `…pre-v2-….bak` appears, schema 2 on dashboard, counts 0; then seeded dir via `WEASEL_DATA_DIR` shows counts.

Commit: `feat(testkit): service fixtures and seed-demo; wire domain services into app state`

### M9 — Docs and checklist (≈0.25 d)

- `09-parity-checklist.md` **Data safety**: tick with evidence (test names) — migrations from zero, backup + restore exercised, FK clean after writes, forced mid-tx failure, history/snapshots. Mark the two legacy items "void (ADR 0006)". Do **not** tick read/write/reconciliation UI items; add an "evidence (backend)" sub-bullet under the ones this plan proves at service level (clipboard failure, idempotent re-import, audit per mutation, removal/restore/conflicts, delta never removes).
- `03-data-model.md`: remove `'legacy'` from the DDL `CHECK`, replace "Legacy data mapping" with a one-line pointer to ADR 0006, add a "Schema as executed" note listing deviations 2–4 below.
- `docs/rewrite/README.md` status: "Phase 2 (data core) implemented — see 03-exec.md". Root README: `seed:demo`, schema version, where backups go.
- ADR 0006 "related decisions": append deviations 5, 7, 8, 9.
- Fill §10. `pnpm verify && pnpm bindings && git diff --exit-code && pnpm test:e2e` green.

Commit: `docs: data model executed; parity checklist evidence`

## 7. Final acceptance (PR description)

- [ ] `0002_domain.sql` applies from zero and on top of a v1 DB; backup taken first; restore test passes.
- [ ] Invariants 1–8 of 03 each have a test (constraint, trigger or service) — list names in the PR.
- [ ] Every write uses `begin_write`; concurrent prepare-send test passes.
- [ ] Every mutation writes audit rows in the same tx; audit is append-only.
- [ ] Clipboard failure records nothing; recopy returns the original snapshot.
- [ ] Re-importing an unchanged sheet is a no-op; stale plans and mid-tx failures roll back fully.
- [ ] Roster safety: empty roster refused, manual contacts never removed, invalid rows protect their person, delta never removes.
- [ ] Planner table + property tests pass.
- [ ] `seed-demo` builds a realistic DB through services only.
- [ ] `app_health` reports counts; bindings current; layer check passes (incl. no tokio in weasel-import).
- [ ] `pnpm verify`, `pnpm test:e2e` green; CI green or explicitly unverified.

## 8. Deviations from the planning docs (decided here)

| # | Doc says | This plan | Why |
| --- | --- | --- | --- |
| 1 | Full schema in `0001_initial.sql` (03) | `0002_domain.sql` | Set by 02-exec §9.2 |
| 2 | `source_kind` incl. `legacy` | Dropped | ADR 0006 |
| 3 | 03 DDL | + `sheet_name`, `invalid_count` | Preview/history need them |
| 4 | Invariants 2/3 + append-only audit by convention | Enforced: open-history unique index, snapshot + audit triggers | Cheap, catches bugs at the DB |
| 5 | `identity_key` = hash, configurable in settings | Plain normalized string; fixed `name-v1` recorded in `app_meta` | Debuggable; config + recompute is phase 5 |
| 6 | Planner + parsing split loosely (02 tree) | Plan *types* in core; reading, planner, `ImportService` in weasel-import | Core cannot depend on weasel-import; import orchestration needs only core ports |
| 7 | `ContactImporter` as a core port (05) | `SourceReader` inside weasel-import | Only weasel-import uses it; keeps core free of parsing |
| 8 | Matching rules 1–4 (03) | + `AmbiguousHistory`, `ContactMatchedTwice`, empty-roster refusal, large-removal warning, invalid-row protection | Never guess, never mass-remove silently |
| 9 | Legacy unsent incl. removed | Audience = active only | 01 defect 2 |
| 10 | Staleness unspecified | Plan carries per-contact `version`; apply is versioned | Preview/apply gap is real |
| 11 | Import progress via events (04) | `ProgressSink` port; 04-exec maps it to `ImportProgress` | Domain stays transport-free |
| 12 | `plan_json` kept | Nulled on discard | PII minimization; applied plans kept for undo |
| 13 | No import error variant in core | `CoreError::Import` | Maps to the existing `IMPORT` code |
| 14 | Hard delete unspecified | Contacts: only without send records; campaigns: only without batches | 03 invariant 7, 06 phase 4 |
| 15 | Preview takes a path (04) | `read_source` (sync) separate from async `preview(ParsedSource)` | Caller controls threading; tests need no files |

## 9. Handoff to 04-exec

Service → command map (04 §Command surface): `ContactService` → `contacts_list/get/create/update/set_status/stats` (+ `contacts_delete` if UI wants it); `CampaignService` → `campaigns_list/get/create/update/set_status/stats/audience`; `SendService` → `campaigns_prepare_send`, `send_batches_list/get/recipients/recopy/set_note`; `ImportService` → `imports_preview` (`validate_source_path` → `spawn_blocking(read_source)` → `preview` with a `ProgressSink` emitting `ImportProgress`), `imports_apply/discard/list/get` (+ `imports_plan_rows`); `AuditRepository` → activity timelines; `search_global` = three small list calls. DTOs: `From` impls in command modules (core types never derive `specta::Type`). Events after commit: `ContactsChanged` (contacts writes, import apply), `CampaignsChanged`, `SendBatchesChanged` (prepare/recopy/note), `ImportsChanged` (preview/apply/discard), `ImportProgress`, `ImportFinished`; add each to `invalidations`. `CoreError::Conflict` from versioned updates drives the "changed elsewhere, reload" dialog.

## 10. Execution log

| Date | Milestone | Note |
| --- | --- | --- |
| 2026-11-11 | M0 | Branch `rewrite/03-data-model`; versions pinned; `weasel-import` deny list includes `tokio`. |
| 2026-11-11 | M1 | `0002_domain.sql`, `begin_write`, `restore_backup`; `d04ec29`. |
| 2026-11-11 | M2 | Domain types, `name-v1`, ports; `34dc98a`. |
| 2026-11-11 | M3 | Contacts, email history, audit; `028d98d`. |
| 2026-11-11 | M4 | Campaigns and clipboard-before-commit sends; `c67a5ea`. |
| 2026-11-11 | M5 | CSV/sheet readers, headers, mapping, normalization; `823123a`. |
| 2026-11-11 | M6 | Reconciliation planner and property tests; `75a50eb`. |
| 2026-11-11 | M7 | Draft/apply/discard import batches; `d8f3fa8`. |
| 2026-11-11 | M8 | Testkit fixtures, `seed-demo`, adapter wiring; `c4243b8`. 50k demo seed 11.47 s; list/search/audience 8/18/16 ms (release, ignored perf test). |
| 2026-11-11 | M9 | Parity evidence, schema-as-executed note, ADR deviations. |

CI has not run: the branch is local and unpushed. `pnpm verify`, `pnpm bindings` +
`git diff --exit-code`, and `pnpm test:e2e` were run locally.
