-- Fixture schema v1 for backup/newer-schema tests.
CREATE TABLE probe (
    id   INTEGER PRIMARY KEY,
    note TEXT
);

-- The real schema always has app_meta; open() writes created_at/instance_id there.
CREATE TABLE app_meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
