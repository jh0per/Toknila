-- name: legacy-16-reviewer-web-domains
-- the domains a project's reviewers may read on the web (a JSON list, empty by default).
ALTER TABLE projects ADD COLUMN reviewer_web_domains TEXT NOT NULL DEFAULT '[]';
