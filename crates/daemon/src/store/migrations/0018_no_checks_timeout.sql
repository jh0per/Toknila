-- name: legacy-18-no-checks-timeout
-- pull-request mode: seconds a pull request may report no check before a person is asked
-- what to do (default ten minutes).
ALTER TABLE projects ADD COLUMN no_checks_timeout_secs INTEGER NOT NULL DEFAULT 600;
