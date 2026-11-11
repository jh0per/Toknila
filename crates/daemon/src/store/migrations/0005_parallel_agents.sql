-- name: legacy-05-parallel-agents
-- parallel agents. The project's limit of agents working at once, and the flag of a
-- Ready card a person started that waits for a free slot (its place in line is computed).
ALTER TABLE projects ADD COLUMN max_parallel INTEGER NOT NULL DEFAULT 2;
ALTER TABLE tickets ADD COLUMN queued INTEGER NOT NULL DEFAULT 0;
