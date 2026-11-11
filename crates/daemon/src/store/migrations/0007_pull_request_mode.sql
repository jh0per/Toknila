-- name: legacy-07-pull-request-mode
-- pull-request mode. A project's merge mode, pull request title template and merge
-- method, and the pull request of a card (JSON: number, url, checks).
ALTER TABLE projects ADD COLUMN merge_mode TEXT NOT NULL DEFAULT 'local';
ALTER TABLE projects ADD COLUMN pr_title TEXT NOT NULL DEFAULT '{key}: {title}';
ALTER TABLE projects ADD COLUMN merge_method TEXT NOT NULL DEFAULT 'merge';
ALTER TABLE tickets ADD COLUMN pr_json TEXT;
