-- name: legacy-14-canvas-position
-- where a person put a card on its chain's canvas (both or neither).
ALTER TABLE tickets ADD COLUMN pos_x INTEGER;
ALTER TABLE tickets ADD COLUMN pos_y INTEGER;
