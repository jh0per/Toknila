-- name: legacy-24-design-pins
-- the assistant's pins on a project's designs (a question on a region, and the person's
-- answer). The design and the card are kept by id and key, not as references: a pin outlives
-- the design file it was made on.
CREATE TABLE design_pins (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id),
    design_id TEXT NOT NULL,
    design_name TEXT NOT NULL,
    number INTEGER NOT NULL,
    region_json TEXT NOT NULL,
    text TEXT NOT NULL,
    card TEXT,
    answer TEXT,
    created_at INTEGER NOT NULL,
    answered_at INTEGER
);
CREATE INDEX design_pins_by_project ON design_pins (project_id, created_at);
