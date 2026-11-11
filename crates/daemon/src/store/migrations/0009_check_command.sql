-- name: legacy-09-check-command
-- the project's check command.
ALTER TABLE projects ADD COLUMN check_command TEXT;
