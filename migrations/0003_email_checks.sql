-- Invalid addresses: per-address verdicts and wildcard rules.
-- Never edit after it has shipped; changes go in a new migration.

-- One verdict per address. 'invalid' puts the address on hold (by hand or after a
-- bounce); 'valid' overrides every rule that matches it.
CREATE TABLE email_flags (
    email_normalized TEXT    PRIMARY KEY,
    email            TEXT    NOT NULL,
    verdict          TEXT    NOT NULL CHECK (verdict IN ('invalid', 'valid')),
    reason           TEXT    NOT NULL CHECK (reason IN ('manual', 'bounced')),
    created_at       INTEGER NOT NULL
) WITHOUT ROWID;

-- Lower-cased SQLite GLOB patterns over the normalized address, using only * and ?.
CREATE TABLE invalid_email_rules (
    id         INTEGER PRIMARY KEY,
    pattern    TEXT    NOT NULL UNIQUE,
    created_at INTEGER NOT NULL
);
