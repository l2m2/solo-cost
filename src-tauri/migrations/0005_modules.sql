-- Project modules + tasks.module_id
CREATE TABLE modules (
  id           INTEGER PRIMARY KEY AUTOINCREMENT,
  project_id   INTEGER NOT NULL REFERENCES projects(id),
  name         TEXT    NOT NULL,
  sort_order   INTEGER NOT NULL DEFAULT 0,
  created_at   TEXT    NOT NULL DEFAULT (datetime('now')),
  updated_at   TEXT    NOT NULL DEFAULT (datetime('now')),
  deleted_at   TEXT
);
CREATE INDEX idx_modules_project ON modules(project_id, deleted_at);

ALTER TABLE tasks ADD COLUMN module_id INTEGER REFERENCES modules(id);
CREATE INDEX idx_tasks_module ON tasks(module_id) WHERE module_id IS NOT NULL;
