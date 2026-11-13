-- name: toknila-122-hidden-project
-- a project left out of the sidebar's list; it works as before.
ALTER TABLE projects ADD COLUMN hidden INTEGER NOT NULL DEFAULT 0;
