-- name: toknila-159-artifact-command
-- the command a release runs on this computer after the tag (the Build step).
ALTER TABLE projects ADD COLUMN artifact_command TEXT;
