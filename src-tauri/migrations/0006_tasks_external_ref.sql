-- Tasks external ref for zentao / future CSV imports
ALTER TABLE tasks ADD COLUMN external_ref TEXT;
CREATE UNIQUE INDEX idx_tasks_external_ref
    ON tasks(project_id, external_ref)
    WHERE external_ref IS NOT NULL AND deleted_at IS NULL;
