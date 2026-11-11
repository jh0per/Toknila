-- name: legacy-20-bump-command
-- the command a release runs to bump the version, and the git tag a release run made.
ALTER TABLE projects ADD COLUMN bump_command TEXT;
ALTER TABLE releases ADD COLUMN tag TEXT;
