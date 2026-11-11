-- name: legacy-15-design-first
-- a card whose agent must show a design for approval before any code.
ALTER TABLE tickets ADD COLUMN design_first INTEGER NOT NULL DEFAULT 0;
