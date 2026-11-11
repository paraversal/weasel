# 04 — IPC contract and events

The IPC boundary is the public API of the desktop app. It is defined in Rust, exported to
TypeScript, and committed. The frontend imports generated names and payload types; it
never hand-writes an invoke name.

**Status:** IPC surface implemented (apiVersion `0.2.0`) — see `04-exec.md` §10 for the
execution log and §8 for every deviation from the plan below.

## Conventions

- Command names are `<feature>_<verb>` in snake_case: `contacts_list`,
  `campaigns_prepare_send`. The generated TypeScript client wraps them in camelCase
  functions (`commands.contactsList`), so both spellings in this document are intentional.
- Payloads and DTOs use `camelCase` in TypeScript (`#[serde(rename_all = "camelCase")]`).
- Every command is `async` and returns `Result<T, AppError>`.
- Requests are explicit structs, even for one field, so adding an optional field is not
  a breaking change.
- Lists are paginated with `limit`/`offset`. `limit` is capped at 500 and defaults to
  100. Responses include `total` so the UI can render a pager.
- Sort fields and filter enums are allowlisted; arbitrary column names are rejected.
- Commands are idempotent where practical. Mutations that create a batch accept no
  client-generated ids; the backend owns identity.
- Timestamps are Unix milliseconds UTC in both directions.

## Contract as executed

The command list below is generated from `commands::builder()` and the DTOs from
`desktop/src-tauri/src/commands/dto.rs`; `desktop/src/lib/ipc/bindings.ts` is committed.

### Deviations from the planning docs

These are the same 13 deviations recorded in `04-exec.md` §8.

| # | Doc said | Executed | Why |
| --- | --- | --- | --- |
| 1 | `revision` inside each event variant | Envelope `WeaselEvent {revision, event}` on `weasel-event` | Shipped by 02-exec; one place to dedupe |
| 2 | Preview takes a path directly | Added `imports_inspect` (no draft) | Incomplete mapping has no other way to show headers |
| 3 | `imports_preview {path, mapping?, scope}` | Added `sheet?`, `progressId?`, `replacesBatchId?` | Sheet choice, progress correlation, no draft pile-up |
| 4 | Any path accepted | Only paths returned by `imports_pick_source` | Webview must not read arbitrary files |
| 5 | `campaigns_audience` → `Page<ContactDto>` | `PageDto<AudienceMemberDto>` | Sent rows are snapshots, possibly of deleted contacts |
| 6 | `campaigns_prepare_send {campaignId}` → `{…, copied}` | Added `expectedCount?`; no `copied` | Guarantees "exactly the previewed set"; success implies copied |
| 7 | `ContactDetailDto` flattens the contact | Nested `contact` | Specta flatten is fragile |
| 8 | `limit`/`offset` required | Optional, default 100/0 | Deserialization errors bypass `AppError` (§4.3) |
| 9 | `ImportProgress.phase` `parsing` | `reading` | Core `ImportPhase` |
| 10 | `ImportFinished.status` `failed` | `applied`/`discarded` only | A failed apply leaves the draft; the error is the response |
| 11 | No hard delete commands | `contacts_delete`, `campaigns_delete` (guarded) | 06 phase 4; owner default §2.2 |
| 12 | Audit timeline unspecified | `activity_list`/`activity_recent` | 06 phase 4 activity timeline |
| 13 | `send_batches_get` → `SendBatchDetailDto` | `SendBatchDto`; recipients paged separately | Batches can hold thousands of recipients |

### App, settings and search

| Command | Request | Response |
| --- | --- | --- |
| `app_info` | — | `{ name, version, tauriVersion, apiVersion, dbPath, dataDir }` |
| `app_health` | — | `{ dbOk, schemaVersion, contactCount, pendingImports, bootError }` |
| `app_features` | — | `FeatureDescriptorDto[]` (`{ id, title, commands, events }`) |
| `app_backup_create` | — | `{ path, bytes, createdAt }` |
| `app_open_data_dir` | — | `void` |
| `settings_get_all` | — | `{ theme, featureFlags }` |
| `settings_update` | `{ patch: { theme?, featureFlags? } }` | `{ theme, featureFlags }` |
| `search_global` | `{ term, limit? }` | `{ contacts, campaigns, sendBatches }` |

`search_global` trims `term`; a blank term returns three empty arrays without touching the
database. `limit` defaults to 5 and is clamped to 1..=20. `app_info` and `app_health` work
even when boot failed, so a bug report can include the database path and error.

### Contacts

| Command | Request | Response |
| --- | --- | --- |
| `contacts_list` | `{ search?, statuses?, importBatchId?, sort?, limit?, offset? }` | `PageDto<ContactDto>` |
| `contacts_get` | `{ id }` | `{ contact, emailHistory, campaigns, metadata }` |
| `contacts_create` | `{ firstName, lastName, email, notes? }` | `ContactDto` (source `manual`) |
| `contacts_update` | `{ id, version, firstName?, lastName?, email?, notes? }` | `ContactDto` |
| `contacts_set_status` | `{ id, version, status }` | `ContactDto` |
| `contacts_delete` | `{ id, version }` | `void` |
| `contacts_stats` | — | `{ active, removed, archived, total, byBatch: [{ batchId, sourceName, count }] }` |

Default list statuses are `active` and `removed`; archived contacts are opt-in. Email
changes through `contacts_update` append `contact_email_history`. Validation rejects an
email already used by another live contact with `CONFLICT`. `contacts_delete` is a hard
delete guarded by version and ownership rules; it is not a cascade for send history.

### Campaigns

| Command | Request | Response |
| --- | --- | --- |
| `campaigns_list` | `{ status?, search?, limit?, offset? }` | `PageDto<CampaignSummaryDto>` |
| `campaigns_get` | `{ id }` | `CampaignDto` |
| `campaigns_create` | `{ name, description?, subject?, body? }` | `CampaignDto` |
| `campaigns_update` | `{ id, version, name?, description?, subject?, body? }` | `CampaignDto` |
| `campaigns_set_status` | `{ id, version, status }` | `CampaignDto` |
| `campaigns_delete` | `{ id, version }` | `void` |
| `campaigns_stats` | `{ id }` | `{ total, sent, unsent, removed, lastBatchAt }` |
| `campaigns_audience` | `{ campaignId, filter: 'sent'\|'unsent'\|'all', search?, limit?, offset? }` | `PageDto<AudienceMemberDto>` |
| `campaigns_prepare_send` | `{ campaignId, expectedCount? }` | `{ batchId, recipientCount, emails }` |

`campaigns_prepare_send` selects active contacts with no send record for the campaign,
writes the clipboard, records one `send_batches` row plus one `send_records` row per
recipient, and returns the batch. `expectedCount` is compared after selection; a mismatch
returns `CONFLICT` with "review the audience again", so a stale preview cannot send.
Clipboard failure returns `IO`, records nothing and emits no event. No unsent contacts is
`INVALID_STATE`. `CampaignSummaryDto` carries the campaign and its stats from one batched
query per page.

### Send history

| Command | Request | Response |
| --- | --- | --- |
| `send_batches_list` | `{ campaignId?, search?, limit?, offset? }` | `PageDto<SendBatchDto>` |
| `send_batches_get` | `{ id }` | `SendBatchDto` |
| `send_batches_recipients` | `{ id, search?, limit?, offset? }` | `PageDto<SendRecordDto>` |
| `send_batches_recopy` | `{ id }` | `{ batchId, recipientCount, emails }` |
| `send_batches_set_note` | `{ id, note }` | `SendBatchDto` |

`SendRecordDto` includes the snapshotted `firstName`, `lastName`, `email`, `status` and
optional `contactId`, so history remains correct after contact edits/removals.

### Imports

| Command | Request | Response |
| --- | --- | --- |
| `imports_pick_source` | — | `SourceFileDto \| null` (`path`, `name`, `kind`, `bytes`) |
| `imports_inspect` | `{ path, sheet? }` | `SourceInspectionDto` (headers, suggested mapping, first 10 sample rows, count) |
| `imports_preview` | `{ path, sheet?, mapping?, scope, progressId?, replacesBatchId? }` | `ImportPreviewDto` |
| `imports_apply` | `{ batchId, resolutions, progressId? }` | `ImportBatchDto` |
| `imports_discard` | `{ batchId }` | `ImportBatchDto` |
| `imports_list` | `{ status?, limit?, offset? }` | `PageDto<ImportBatchDto>` |
| `imports_get` | `{ batchId, limit?, offset? }` | `{ batch, members: PageDto<ContactDto> }` |
| `imports_plan_rows` | `{ batchId, limit?, offset? }` | `PageDto<PlannedRowDto>` |

`imports_pick_source` is the only command that grants read access: it canonicalizes the
dialog path and inserts it into `PickedSources`. Every path-taking import command calls
`ensure_picked`, which canonicalizes and returns
`VALIDATION {field: "path", reason: "Choose the file again"}` for anything else.
`imports_preview` creates a `draft` batch and persists its plan; re-preview with
`replacesBatchId` discards the old draft and adds a `sameFileAsBatch` warning when the
file hash matches. `imports_apply` re-validates the draft and applies non-conflicting
actions plus the caller's resolutions in one transaction; unresolved conflicts are
`INVALID_STATE`. `imports_plan_rows` rejects a discarded batch and reads the stored plan;
applied imports keep their plan for review.

`ImportPreviewDto` contains `batch`, `headers`, `headerRow`, `suggestedMapping`,
`mapping`, `summary`, `sample`, `conflicts`, `errors`, `warnings` and `removals`.
Conflict resolution:

```ts
type ConflictResolutionDto = {
  row: number;                       // source row index
  action: "linkContact" | "createNew" | "skip";
  contactId?: number;                // required for linkContact
};
```

`ImportSummaryDto` is `{ rowCount, added, updated, removed, conflicts, unchanged,
invalid }`.

### Activity

| Command | Request | Response |
| --- | --- | --- |
| `activity_recent` | `{ limit?, offset? }` | `PageDto<AuditEventDto>` |
| `activity_list` | `{ entityType, entityId, limit?, offset? }` | `PageDto<AuditEventDto>` |

`AuditEventDto` is `{ id, at, actor, action, entityType, entityId, before, after,
metadata }`. Activity is read-only; audit rows are append-only.

## Type generation

`desktop/src-tauri/src/bindings.rs` builds a `tauri_specta::Builder`, registers exactly
the same command list as `lib.rs`, and exports:

- `desktop/src/lib/ipc/bindings.ts` — command functions, DTO types, event union;
- `desktop/src/lib/ipc/bindings.schema.json` — optional machine-readable contract.

The generated file is committed. CI runs the exporter and fails if `git diff` is dirty,
so a backend signature change cannot merge without regenerating the frontend types.

## Errors

```rust
#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: ErrorCode,
    pub message: String,
    pub details: Option<serde_json::Value>,
    pub retryable: bool,
}

#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    Validation,
    NotFound,
    Conflict,
    InvalidState,
    Io,
    Import,
    Cancelled,
    Internal,
}
```

TypeScript counterpart (generated):

```ts
export type AppError = {
  code: "VALIDATION" | "NOT_FOUND" | "CONFLICT" | "INVALID_STATE"
      | "IO" | "IMPORT" | "CANCELLED" | "INTERNAL";
  message: string;
  details?: Value;
  retryable: boolean;
};
```

`details` is shaped for field errors:

```json
{ "field": "email", "reason": "invalid" }
```

The client wrapper turns a rejected invoke into `AppError`, surfaces `VALIDATION` on the
matching form field, shows `CONFLICT` as a "changed elsewhere, reload" dialog, and toasts
`INTERNAL` with a log id. Mock handlers throw the same plain object, so `typedError`
behaves identically in `VITE_IPC_MOCK=1`.

## Shared DTOs

```rust
#[derive(Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PageDto<T> {
    pub items: Vec<T>,
    pub total: i64,
    pub offset: u32,
    pub limit: u32,
}

#[derive(Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ContactDto {
    pub id: i64,
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub status: ContactStatus,
    pub source: ContactSource,
    pub import_batch_id: Option<i64>,
    pub date_added: i64,
    pub updated_at: i64,
    pub version: i64,
    pub notes: String,
}

#[derive(Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ContactDetailDto {
    pub contact: ContactDto,
    pub email_history: Vec<EmailHistoryDto>,
    pub campaigns: Vec<CampaignReceiptDto>,
    pub metadata: serde_json::Value,
}
```

Contacts list request:

```rust
#[derive(Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ContactListRequest {
    pub search: Option<String>,
    pub statuses: Option<Vec<ContactStatus>>,
    pub import_batch_id: Option<i64>,
    pub sort: Option<ContactSort>,    // lastNameAsc | lastNameDesc | addedDesc | emailAsc
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}
```

Every mutation wrapper returns the DTO and emits its events only after the service commits.
The wrapper emits `Emitted { value, events }`; the exported command returns `value`.

## Events

There is one multiplexed Tauri event channel, `weasel-event`, carrying an envelope with a
typed union and a monotonically increasing revision:

```rust
#[derive(Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct WeaselEvent {
    pub revision: u64,
    pub event: DomainEvent,
}

#[derive(Clone, Serialize, specta::Type)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum DomainEvent {
    SettingsChanged { keys: Vec<String> },
    OperationFailed { operation: String, error: AppError },
    ContactsChanged,
    CampaignsChanged,
    SendBatchesChanged,
    ImportsChanged,
    ImportProgress {
        progress_id: String,
        phase: ImportPhase,          // reading | matching | applying
        processed: u64,
        total: u64,
    },
    ImportFinished {
        batch_id: i64,
        status: ImportStatus,        // applied | discarded
    },
}
```

The revision is stored in an `AtomicU64` in `AppState` and incremented with `fetch_add`;
the frontend drops envelopes whose revision it has already seen. A new event variant is
a TypeScript build error until `events.ts` lists its invalidations.

Frontend mapping, in `desktop/src/lib/ipc/events.ts`:

| Event | Invalidate |
| --- | --- |
| `contactsChanged` | `['contacts']`, `['campaigns']`, `['search']`, `['activity']`, app health |
| `campaignsChanged` | `['campaigns']`, `['search']`, `['activity']` |
| `sendBatchesChanged` | `['sendBatches']`, `['campaigns']`, `['contacts','detail']`, `['search']`, `['activity']` |
| `importsChanged` | `['imports']`, app health, `['activity']` |
| `settingsChanged` | `['settings']` |
| `importProgress` | no invalidation; `onEvent` updates the progress store |
| `importFinished` | `['imports']`, `['contacts']`, `['campaigns']`, app health |
| `operationFailed` | toast + log; passed to `onFailed`, not `onEvent` |

`startEventBridge(queryClient, onFailed, onEvent?)` returns its disposer. `onEvent`
receives every accepted event after invalidation, which is where the import wizard builds
progress state.

## Compatibility policy

- Additive optional fields and new commands are non-breaking. Generated types update.
- Renaming/removing a command or changing a field type is breaking: add a new command
  (for example `contacts_list_v2`) and keep the old one for one release, or accept the
  coordinated breaking change while the app is pre-1.0.
- `app_info.apiVersion` follows semantic versioning for the IPC surface; this surface is
  `0.2.0` after `app_backup_create` and the final import command set.
- Events are treated as an open union in TypeScript (`default: ignore`) so an older
  frontend tolerates an unknown event during development.
- Database migrations have their own monotonic version; `app_health.schemaVersion`
  exposes it for diagnostics.
