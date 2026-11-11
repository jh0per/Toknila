-- name: legacy-10-archived-projects
-- archived projects. Hidden from the sidebar, data kept.
ALTER TABLE projects ADD COLUMN archived INTEGER NOT NULL DEFAULT 0;
