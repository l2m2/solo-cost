-- Adds the `paused` task status and the task_events timeline table.
-- SQLite cannot alter a CHECK constraint in place, so tasks is rebuilt again
-- (see 0008). The migration runner disables foreign_keys for the whole pass,
-- so dropping tasks while time_logs references it is safe.

CREATE TABLE tasks_new (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id       INTEGER NOT NULL REFERENCES projects(id),
    title            TEXT    NOT NULL,
    description      TEXT,
    assignee_id      INTEGER REFERENCES members(id),
    status           TEXT    NOT NULL DEFAULT 'todo'
                              CHECK (status IN ('todo','in_progress','paused','done','closed')),
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
                       estimated_hours, due_date, started_at, completed_at,
                       module_id, external_ref, created_at, updated_at, deleted_at)
SELECT id, project_id, title, description, assignee_id, status,
       estimated_hours, due_date, started_at, completed_at,
       module_id, external_ref, created_at, updated_at, deleted_at
FROM tasks;

DROP TABLE tasks;
ALTER TABLE tasks_new RENAME TO tasks;

CREATE INDEX idx_tasks_project_status ON tasks(project_id, status, deleted_at);
CREATE INDEX idx_tasks_assignee ON tasks(assignee_id, deleted_at);
CREATE INDEX idx_tasks_module ON tasks(module_id) WHERE module_id IS NOT NULL;
CREATE UNIQUE INDEX idx_tasks_external_ref
    ON tasks(project_id, external_ref)
    WHERE external_ref IS NOT NULL AND deleted_at IS NULL;

-- One table carries both hand-written notes and status transitions so the
-- detail page can render them as a single narrative.
-- occurred_at is when the fact happened (users may backdate a start time);
-- created_at is when the row was written. The timeline sorts by the former.
CREATE TABLE task_events (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id      INTEGER NOT NULL REFERENCES tasks(id),
    kind         TEXT    NOT NULL CHECK (kind IN ('note','status_change')),
    from_status  TEXT,
    to_status    TEXT,
    body         TEXT,
    occurred_at  TEXT    NOT NULL,
    created_at   TEXT    NOT NULL DEFAULT (datetime('now')),
    deleted_at   TEXT
);

CREATE INDEX idx_task_events_task ON task_events(task_id, deleted_at, occurred_at);

-- Backfill: synthesise history from the date columns tasks already carry, so
-- pre-existing and zentao-imported tasks do not open onto an empty timeline.
INSERT INTO task_events (task_id, kind, from_status, to_status, occurred_at, created_at)
SELECT id, 'status_change', NULL, 'todo', created_at, created_at
FROM tasks WHERE deleted_at IS NULL;

INSERT INTO task_events (task_id, kind, from_status, to_status, occurred_at, created_at)
SELECT id, 'status_change', 'todo', 'in_progress', started_at, started_at
FROM tasks WHERE deleted_at IS NULL AND started_at IS NOT NULL;

INSERT INTO task_events (task_id, kind, from_status, to_status, occurred_at, created_at)
SELECT id, 'status_change',
       CASE WHEN started_at IS NOT NULL THEN 'in_progress' ELSE 'todo' END,
       CASE WHEN status = 'closed' THEN 'closed' ELSE 'done' END,
       completed_at, completed_at
FROM tasks WHERE deleted_at IS NULL AND completed_at IS NOT NULL;

-- Catch-all: a task whose current status produced no event above (e.g. imported
-- as 'done' with no completed_at) would show a timeline contradicting its badge.
-- occurred_at uses MAX(created_at, started_at) rather than created_at alone:
-- a task can be 'done' with started_at set but completed_at NULL (zentao import
-- with a blank finish date), and created_at alone would place this catch-all
-- event before the in_progress event seeded above it, showing the task as
-- finished before it started.
INSERT INTO task_events (task_id, kind, from_status, to_status, occurred_at, created_at)
SELECT t.id, 'status_change', NULL, t.status,
       MAX(t.created_at, COALESCE(t.started_at, t.created_at)), t.created_at
FROM tasks t
WHERE t.deleted_at IS NULL
  AND NOT EXISTS (
      SELECT 1 FROM task_events e WHERE e.task_id = t.id AND e.to_status = t.status
  );
