-- name: legacy-21-ticket-designs
-- the designs a card references (a JSON list of design id and optional region).
ALTER TABLE tickets ADD COLUMN designs_json TEXT;
