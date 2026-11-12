-- name: toknila-86-ticket-model
-- the card's own model; NULL means the project's worker model.
ALTER TABLE tickets ADD COLUMN model TEXT;
