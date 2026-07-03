-- Clients as first-class entities; projects reference by FK.
-- Existing projects.client_name text is discarded per product decision:
-- users will re-bind projects to real client entities after this migration.

CREATE TABLE clients (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    company_id INTEGER NOT NULL REFERENCES companies(id),
    name TEXT NOT NULL,
    contact_name TEXT,
    contact_info TEXT,
    tax_id TEXT,
    legal_name TEXT,
    notes TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    deleted_at TEXT
);

CREATE UNIQUE INDEX idx_clients_company_name_unique
    ON clients(company_id, lower(name))
    WHERE deleted_at IS NULL;

ALTER TABLE projects ADD COLUMN client_id INTEGER REFERENCES clients(id);
ALTER TABLE projects DROP COLUMN client_name;

CREATE INDEX idx_projects_client_id ON projects(client_id) WHERE deleted_at IS NULL;
