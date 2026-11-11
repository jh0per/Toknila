-- name: legacy-06-review
-- review. A ticket's review state (whose turn, the rounds so far; JSON) and the
-- reviewer agent profile that may be set on a ticket, a chain and a project.
ALTER TABLE tickets ADD COLUMN review_json TEXT;
ALTER TABLE tickets ADD COLUMN reviewer TEXT;
ALTER TABLE chains ADD COLUMN reviewer TEXT;
ALTER TABLE projects ADD COLUMN default_reviewer TEXT;
