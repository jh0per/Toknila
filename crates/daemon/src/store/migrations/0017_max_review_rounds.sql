-- name: legacy-17-max-review-rounds
-- how many times the loop (check, reviewer) may return a card before a person decides.
-- Existing projects keep their review mode; only the limit is new.
ALTER TABLE projects ADD COLUMN max_review_rounds INTEGER NOT NULL DEFAULT 3;
