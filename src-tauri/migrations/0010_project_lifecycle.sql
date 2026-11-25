-- Keep the old archive flag without inventing a business status for legacy rows.
DROP INDEX idx_projects_company_status;
ALTER TABLE projects ADD COLUMN is_archived INTEGER NOT NULL DEFAULT 0 CHECK (is_archived IN (0,1));
ALTER TABLE projects ADD COLUMN business_status TEXT NOT NULL DEFAULT 'pending'
    CHECK (business_status IN ('negotiating','pending','in_progress','delivered','settled','cancelled','unknown'));
UPDATE projects SET is_archived = (status = 'archived'),
    business_status = CASE WHEN status = 'archived' THEN 'unknown' ELSE status END;
ALTER TABLE projects DROP COLUMN status;
ALTER TABLE projects RENAME COLUMN business_status TO status;
CREATE INDEX idx_projects_company_status ON projects(company_id, is_archived, status, deleted_at);
ALTER TABLE projects ADD COLUMN cancelled_from TEXT
    CHECK (cancelled_from IN ('negotiating','pending','in_progress'));

CREATE TABLE project_status_events (
    id INTEGER PRIMARY KEY,
    project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    action TEXT NOT NULL,
    from_status TEXT,
    to_status TEXT NOT NULL,
    reason TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_project_status_events ON project_status_events(project_id, id);
CREATE TRIGGER projects_record_creation AFTER INSERT ON projects BEGIN
    INSERT INTO project_status_events(project_id, action, to_status)
    VALUES (NEW.id, 'created', NEW.status);
END;
