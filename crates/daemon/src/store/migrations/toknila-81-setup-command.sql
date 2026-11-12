-- name: toknila-81-setup-command
-- the project's setup command, run once in every new ship.
ALTER TABLE projects ADD COLUMN setup_command TEXT;
