-- Add `closed` task status + real source dates (started_at / completed_at).
-- SQLite cannot alter a CHECK constraint in place, so the tasks table is rebuilt.
-- The migration runner disables foreign_keys for the whole migration pass, so
-- dropping the tasks table (referenced by time_logs) does not trip NO ACTION checks.

CREATE TABLE tasks_new (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id       INTEGER NOT NULL REFERENCES projects(id),
    title            TEXT    NOT NULL,
    description      TEXT,
    assignee_id      INTEGER REFERENCES members(id),
    status           TEXT    NOT NULL DEFAULT 'todo'
                              CHECK (status IN ('todo','in_progress','done','closed')),
    estimated_hours  REAL    CHECK (estimated_hours IS NULL OR (estimated_hours >= 0 AND estimated_hours <= 9999)),
    due_date         TEXT,
    started_at       TEXT,
    completed_at     TEXT,
    module_id        INTEGER REFERENCES modules(id),
    external_ref     TEXT,
    created_at       TEXT    NOT NULL DEFAULT (datetime('now')),
    updated_at       TEXT    NOT NULL DEFAULT (datetime('now')),
    deleted_at       TEXT
);

INSERT INTO tasks_new (id, project_id, title, description, assignee_id, status,
                       estimated_hours, due_date, module_id, external_ref,
                       created_at, updated_at, deleted_at)
SELECT id, project_id, title, description, assignee_id, status,
       estimated_hours, due_date, module_id, external_ref,
       created_at, updated_at, deleted_at
FROM tasks;

DROP TABLE tasks;
ALTER TABLE tasks_new RENAME TO tasks;

CREATE INDEX idx_tasks_project_status ON tasks(project_id, status, deleted_at);
CREATE INDEX idx_tasks_assignee ON tasks(assignee_id, deleted_at);
CREATE INDEX idx_tasks_module ON tasks(module_id) WHERE module_id IS NOT NULL;
CREATE UNIQUE INDEX idx_tasks_external_ref
    ON tasks(project_id, external_ref)
    WHERE external_ref IS NOT NULL AND deleted_at IS NULL;
