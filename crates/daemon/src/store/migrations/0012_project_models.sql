-- name: legacy-12-project-models
-- the Claude model of a project's card agents, reviewers and planner.
ALTER TABLE projects ADD COLUMN worker_model TEXT;
ALTER TABLE projects ADD COLUMN reviewer_model TEXT;
ALTER TABLE projects ADD COLUMN planner_model TEXT;
