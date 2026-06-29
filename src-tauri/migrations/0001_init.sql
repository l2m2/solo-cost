-- Application metadata: key-value pairs.
-- Uses IF NOT EXISTS because the migration runner ensures this table exists before applying migrations.
CREATE TABLE IF NOT EXISTS app_meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

INSERT INTO app_meta (key, value) VALUES ('schema_version', '1');
INSERT INTO app_meta (key, value) VALUES ('default_currency', 'CNY');
INSERT INTO app_meta (key, value) VALUES ('auto_lock_minutes', '15');

-- Companies table
CREATE TABLE companies (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    name              TEXT    NOT NULL,
    legal_name        TEXT,
    tax_id            TEXT,
    default_tax_rate  REAL    NOT NULL DEFAULT 0.06 CHECK (default_tax_rate >= 0 AND default_tax_rate < 1),
    currency_code     TEXT    NOT NULL DEFAULT 'CNY',
    notes             TEXT,
    created_at        TEXT    NOT NULL DEFAULT (datetime('now')),
    updated_at        TEXT    NOT NULL DEFAULT (datetime('now')),
    deleted_at        TEXT
);

CREATE INDEX idx_companies_deleted_at ON companies(deleted_at);
