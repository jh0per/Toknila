-- name: legacy-19-canvas-size
-- the size a person gave a card on its chain's canvas (both or neither).
ALTER TABLE tickets ADD COLUMN size_w INTEGER;
ALTER TABLE tickets ADD COLUMN size_h INTEGER;
