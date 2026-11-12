-- name: toknila-95-simulated-project
-- a project marked for simulation: its cards may run under the `sim` profile (no process).
ALTER TABLE projects ADD COLUMN simulated INTEGER NOT NULL DEFAULT 0;
