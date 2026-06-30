-- M3: members / contract_payments / tasks / time_logs
-- All money values stored as INTEGER cents.
-- All business tables include deleted_at for soft delete with cascade-by-timestamp.
-- FK omits ON DELETE CASCADE intentionally — we soft-delete via domain layer.

CREATE TABLE members (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    company_id        INTEGER NOT NULL REFERENCES companies(id),
    name              TEXT    NOT NULL,
    role              TEXT,
    daily_cost_cents  INTEGER NOT NULL DEFAULT 0 CHECK (daily_cost_cents >= 0),
    effective_from    TEXT,
    is_active         INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0,1)),
    notes             TEXT,
    created_at        TEXT    NOT NULL DEFAULT (datetime('now')),
    updated_at        TEXT    NOT NULL DEFAULT (datetime('now')),
    deleted_at        TEXT
);

CREATE INDEX idx_members_company_active ON members(company_id, is_active, deleted_at);
CREATE INDEX idx_members_deleted_at ON members(deleted_at);

CREATE TABLE contract_payments (
    id                    INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id            INTEGER NOT NULL REFERENCES projects(id),
    name                  TEXT    NOT NULL,
    expected_amount_cents INTEGER NOT NULL DEFAULT 0 CHECK (expected_amount_cents >= 0),
    expected_date         TEXT,
    actual_amount_cents   INTEGER CHECK (actual_amount_cents IS NULL OR actual_amount_cents >= 0),
    actual_received_at    TEXT,
    sort_order            INTEGER NOT NULL DEFAULT 0,
    notes                 TEXT,
    deleted_at            TEXT
);

CREATE INDEX idx_contract_payments_project ON contract_payments(project_id, deleted_at);

CREATE TABLE tasks (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id       INTEGER NOT NULL REFERENCES projects(id),
    title            TEXT    NOT NULL,
    description      TEXT,
    assignee_id      INTEGER REFERENCES members(id),
    status           TEXT    NOT NULL DEFAULT 'todo'
                              CHECK (status IN ('todo','in_progress','done')),
    estimated_hours  REAL    CHECK (estimated_hours IS NULL OR (estimated_hours >= 0 AND estimated_hours <= 9999)),
    due_date         TEXT,
    created_at       TEXT    NOT NULL DEFAULT (datetime('now')),
    updated_at       TEXT    NOT NULL DEFAULT (datetime('now')),
    deleted_at       TEXT
);

CREATE INDEX idx_tasks_project_status ON tasks(project_id, status, deleted_at);
CREATE INDEX idx_tasks_assignee ON tasks(assignee_id, deleted_at);

CREATE TABLE time_logs (
    id                          INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id                     INTEGER NOT NULL REFERENCES tasks(id),
    member_id                   INTEGER NOT NULL REFERENCES members(id),
    work_date                   TEXT    NOT NULL,
    hours                       REAL    NOT NULL CHECK (hours >= 0 AND hours <= 24),
    daily_cost_snapshot_cents   INTEGER NOT NULL CHECK (daily_cost_snapshot_cents >= 0),
    notes                       TEXT,
    created_at                  TEXT    NOT NULL DEFAULT (datetime('now')),
    deleted_at                  TEXT
);

CREATE INDEX idx_time_logs_task ON time_logs(task_id, deleted_at);
CREATE INDEX idx_time_logs_member ON time_logs(member_id, deleted_at);
CREATE INDEX idx_time_logs_work_date ON time_logs(work_date);

-- schema_version bumped to 3 (runner also updates via ON CONFLICT — see 0002 note).
INSERT INTO app_meta(key, value) VALUES ('schema_version', '3')
ON CONFLICT(key) DO UPDATE SET value = excluded.value;
