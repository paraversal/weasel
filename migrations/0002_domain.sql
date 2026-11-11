-- Domain schema v2: contacts, campaigns, sends, imports and audit.
-- Never edit after it has shipped; changes go in a new migration.

-- One row per import/reconciliation run. Draft rows hold preview plans; applied rows
-- are the permanent record of what changed. Defined first because contacts reference it.
CREATE TABLE import_batches (
    id             INTEGER PRIMARY KEY,
    source_kind    TEXT    NOT NULL
                   CHECK (source_kind IN ('xlsx', 'xls', 'ods', 'csv', 'manual')),
    source_name    TEXT    NOT NULL,
    source_path    TEXT,
    file_hash      TEXT,
    sheet_name     TEXT,
    scope          TEXT    NOT NULL DEFAULT 'roster'
                   CHECK (scope IN ('roster', 'delta')),
    status         TEXT    NOT NULL DEFAULT 'draft'
                   CHECK (status IN ('draft', 'applied', 'failed', 'discarded')),
    created_at     INTEGER NOT NULL,
    applied_at     INTEGER,
    row_count      INTEGER NOT NULL DEFAULT 0,
    added_count    INTEGER NOT NULL DEFAULT 0,
    updated_count  INTEGER NOT NULL DEFAULT 0,
    removed_count  INTEGER NOT NULL DEFAULT 0,
    conflict_count INTEGER NOT NULL DEFAULT 0,
    unchanged_count INTEGER NOT NULL DEFAULT 0,
    invalid_count  INTEGER NOT NULL DEFAULT 0,
    plan_json      TEXT CHECK (plan_json IS NULL OR json_valid(plan_json)),
    error          TEXT
);
CREATE INDEX ix_import_batches_created
    ON import_batches(created_at DESC);
CREATE INDEX ix_import_batches_status
    ON import_batches(status);

-- Contacts: the domain entity behind the "Freshies" screens.
CREATE TABLE contacts (
    id                INTEGER PRIMARY KEY,
    first_name        TEXT    NOT NULL,
    last_name         TEXT    NOT NULL,
    email             TEXT    NOT NULL,
    email_normalized  TEXT    NOT NULL,
    identity_key      TEXT,
    status            TEXT    NOT NULL DEFAULT 'active'
                      CHECK (status IN ('active', 'removed', 'archived')),
    source            TEXT    NOT NULL DEFAULT 'import'
                      CHECK (source IN ('import', 'manual', 'api')),
    import_batch_id   INTEGER REFERENCES import_batches(id) ON DELETE SET NULL,
    date_added        INTEGER NOT NULL,
    updated_at        INTEGER NOT NULL,
    version           INTEGER NOT NULL DEFAULT 1,
    notes             TEXT    NOT NULL DEFAULT '',
    metadata_json     TEXT    NOT NULL DEFAULT '{}'
                      CHECK (json_valid(metadata_json))
);

-- One live address per contact. Removed people keep their address so they can be
-- restored; archived people are out of the way and do not block a new person.
CREATE UNIQUE INDEX ux_contacts_email_live
    ON contacts(email_normalized)
    WHERE status IN ('active', 'removed');

CREATE INDEX ix_contacts_name
    ON contacts(last_name, first_name);
CREATE INDEX ix_contacts_batch
    ON contacts(import_batch_id);
CREATE INDEX ix_contacts_identity
    ON contacts(identity_key)
    WHERE identity_key IS NOT NULL;
CREATE INDEX ix_contacts_status
    ON contacts(status);

-- Every address a contact has ever had. Used to match a person whose email changed,
-- and to explain why the change happened.
CREATE TABLE contact_email_history (
    id               INTEGER PRIMARY KEY,
    contact_id       INTEGER NOT NULL REFERENCES contacts(id) ON DELETE CASCADE,
    email            TEXT    NOT NULL,
    email_normalized TEXT    NOT NULL,
    valid_from       INTEGER NOT NULL,
    valid_to         INTEGER,
    reason           TEXT    NOT NULL
                     CHECK (reason IN ('import', 'manual', 'merge', 'correction'))
);
CREATE INDEX ix_contact_email_history_lookup
    ON contact_email_history(email_normalized);
CREATE INDEX ix_contact_email_history_contact
    ON contact_email_history(contact_id, valid_from DESC);
-- Invariant 2: at most one open history row per contact.
CREATE UNIQUE INDEX ux_email_history_open
    ON contact_email_history(contact_id)
    WHERE valid_to IS NULL;

-- Campaigns. `description` is the old `info` note; `body` is the old `text`.
CREATE TABLE campaigns (
    id          INTEGER PRIMARY KEY,
    name        TEXT    NOT NULL,
    description TEXT    NOT NULL DEFAULT '',
    subject     TEXT    NOT NULL DEFAULT '',
    body        TEXT    NOT NULL DEFAULT '',
    status      TEXT    NOT NULL DEFAULT 'active'
                CHECK (status IN ('draft', 'active', 'archived')),
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL,
    version     INTEGER NOT NULL DEFAULT 1
);
CREATE INDEX ix_campaigns_status ON campaigns(status, name);

-- A "copy" is a send batch: one preparation of recipients for a campaign.
CREATE TABLE send_batches (
    id              INTEGER PRIMARY KEY,
    campaign_id     INTEGER NOT NULL REFERENCES campaigns(id) ON DELETE RESTRICT,
    created_at      INTEGER NOT NULL,
    recipient_count INTEGER NOT NULL,
    note            TEXT    NOT NULL DEFAULT '',
    copied_at       INTEGER
);
CREATE INDEX ix_send_batches_campaign
    ON send_batches(campaign_id, created_at DESC);

-- Immutable recipient records. Name/email are snapshots: editing or deleting a contact
-- must never rewrite history.
CREATE TABLE send_records (
    id               INTEGER PRIMARY KEY,
    batch_id         INTEGER NOT NULL REFERENCES send_batches(id) ON DELETE CASCADE,
    contact_id       INTEGER REFERENCES contacts(id) ON DELETE SET NULL,
    email            TEXT    NOT NULL,
    email_normalized TEXT    NOT NULL,
    first_name       TEXT    NOT NULL,
    last_name        TEXT    NOT NULL,
    status           TEXT    NOT NULL DEFAULT 'created'
                     CHECK (status IN ('created', 'copied', 'sent', 'failed', 'skipped')),
    created_at       INTEGER NOT NULL,
    UNIQUE (batch_id, contact_id)
);
CREATE INDEX ix_send_records_batch  ON send_records(batch_id);
CREATE INDEX ix_send_records_contact ON send_records(contact_id);
CREATE INDEX ix_send_records_email  ON send_records(email_normalized);

-- Invariant 3: the recipient snapshot never changes; only the status/link may.
CREATE TRIGGER send_records_snapshot_immutable
BEFORE UPDATE OF batch_id, email, email_normalized, first_name, last_name ON send_records
BEGIN
    SELECT RAISE(ABORT, 'send record snapshot is immutable');
END;

-- Append-only audit trail. Also powers activity timelines and a future undo stack.
CREATE TABLE audit_events (
    id            INTEGER PRIMARY KEY,
    at            INTEGER NOT NULL,
    actor         TEXT    NOT NULL DEFAULT 'local',
    action        TEXT    NOT NULL,
    entity_type   TEXT    NOT NULL,
    entity_id     TEXT,
    before_json   TEXT CHECK (before_json IS NULL OR json_valid(before_json)),
    after_json    TEXT CHECK (after_json IS NULL OR json_valid(after_json)),
    metadata_json TEXT    NOT NULL DEFAULT '{}'
                  CHECK (json_valid(metadata_json))
);
CREATE INDEX ix_audit_entity
    ON audit_events(entity_type, entity_id, at DESC);
CREATE INDEX ix_audit_time
    ON audit_events(at DESC);

CREATE TRIGGER audit_events_no_update
BEFORE UPDATE ON audit_events
BEGIN
    SELECT RAISE(ABORT, 'audit log is append-only');
END;

CREATE TRIGGER audit_events_no_delete
BEFORE DELETE ON audit_events
BEGIN
    SELECT RAISE(ABORT, 'audit log is append-only');
END;
