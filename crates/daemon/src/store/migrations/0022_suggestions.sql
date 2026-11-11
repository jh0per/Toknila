-- name: legacy-22-suggestions
-- the planner's standing suggestion, one per project (the card, chain and action are
-- kept apart from the JSON so an event can find the suggestions it ends).
CREATE TABLE suggestions (
    project_id TEXT PRIMARY KEY REFERENCES projects(id),
    card TEXT,
    chain_id TEXT,
    action TEXT,
    json TEXT NOT NULL
);
