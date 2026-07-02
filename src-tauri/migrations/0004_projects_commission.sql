-- Project sales commission fields

ALTER TABLE projects ADD COLUMN commission_mode TEXT NOT NULL DEFAULT 'none';
ALTER TABLE projects ADD COLUMN commission_rate REAL;
ALTER TABLE projects ADD COLUMN commission_amount_cents INTEGER;
ALTER TABLE projects ADD COLUMN commission_settled INTEGER NOT NULL DEFAULT 0;
