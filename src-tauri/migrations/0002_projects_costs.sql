-- M2: cost_categories / projects / cost_entries
-- All money values stored as INTEGER cents.
-- All business tables include deleted_at for soft delete with cascade-by-timestamp.

CREATE TABLE cost_categories (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    company_id   INTEGER NOT NULL REFERENCES companies(id),
    name         TEXT    NOT NULL,
    is_system    INTEGER NOT NULL DEFAULT 0 CHECK (is_system IN (0, 1)),
    sort_order   INTEGER NOT NULL DEFAULT 0,
    deleted_at   TEXT
);

CREATE INDEX idx_cost_categories_company ON cost_categories(company_id, deleted_at);

CREATE TABLE projects (
    id                                INTEGER PRIMARY KEY AUTOINCREMENT,
    company_id                        INTEGER NOT NULL REFERENCES companies(id),
    name                              TEXT    NOT NULL,
    client_name                       TEXT,
    status                            TEXT    NOT NULL DEFAULT 'pending'
                                              CHECK (status IN ('negotiating','pending','in_progress',
                                                                'delivered','settled','archived')),
    contract_amount_cents             INTEGER NOT NULL DEFAULT 0 CHECK (contract_amount_cents >= 0),
    contract_amount_is_tax_inclusive  INTEGER NOT NULL DEFAULT 1 CHECK (contract_amount_is_tax_inclusive IN (0,1)),
    tax_rate                          REAL    NOT NULL DEFAULT 0.06 CHECK (tax_rate >= 0 AND tax_rate < 1),
    start_date                        TEXT,
    end_date                          TEXT,
    actual_delivered_at               TEXT,
    notes                             TEXT,
    created_at                        TEXT    NOT NULL DEFAULT (datetime('now')),
    updated_at                        TEXT    NOT NULL DEFAULT (datetime('now')),
    deleted_at                        TEXT
);

CREATE INDEX idx_projects_company_status ON projects(company_id, status, deleted_at);
CREATE INDEX idx_projects_deleted_at ON projects(deleted_at);

CREATE TABLE cost_entries (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id    INTEGER NOT NULL REFERENCES projects(id),
    category_id   INTEGER NOT NULL REFERENCES cost_categories(id),
    incurred_at   TEXT    NOT NULL,
    amount_cents  INTEGER NOT NULL CHECK (amount_cents >= 0),
    description   TEXT,
    notes         TEXT,
    created_at    TEXT    NOT NULL DEFAULT (datetime('now')),
    deleted_at    TEXT
);

CREATE INDEX idx_cost_entries_project ON cost_entries(project_id, deleted_at);
CREATE INDEX idx_cost_entries_category ON cost_entries(category_id, deleted_at);
CREATE INDEX idx_cost_entries_incurred ON cost_entries(incurred_at);

-- schema_version is rewritten by the migration runner's INSERT ... ON CONFLICT DO UPDATE,
-- but stating the intent here documents the migration boundary.
INSERT INTO app_meta(key, value) VALUES ('schema_version', '2')
ON CONFLICT(key) DO UPDATE SET value = excluded.value;
