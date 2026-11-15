-- name: toknila-130-extra-checks
-- the project's extra checks: a JSON list of {paths, command} that run after the check command
-- for the files a card changed.
ALTER TABLE projects ADD COLUMN extra_checks TEXT NOT NULL DEFAULT '[]';
