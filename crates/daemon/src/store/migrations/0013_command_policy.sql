-- name: legacy-13-command-policy
-- when a card agent wants to run a command (existing projects keep asking) and the rules
-- a person remembered with "Allow always" (a JSON list).
ALTER TABLE projects ADD COLUMN command_policy TEXT NOT NULL DEFAULT 'ask';
ALTER TABLE projects ADD COLUMN allowed_commands TEXT NOT NULL DEFAULT '[]';
