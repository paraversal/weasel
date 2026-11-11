-- Application shell only. Domain tables are added by 03-exec as 0002_domain.sql.
-- Never edit this file after it has shipped; changes go in a new migration.

-- Flat key/value settings. Values are JSON so each key can evolve independently.
CREATE TABLE settings (
    key        TEXT PRIMARY KEY,
    value_json TEXT    NOT NULL CHECK (json_valid(value_json)),
    updated_at INTEGER NOT NULL
);

-- Instance metadata (created once) for diagnostics and backups.
CREATE TABLE app_meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
