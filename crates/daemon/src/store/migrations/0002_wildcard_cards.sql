-- name: legacy-02-wildcard-cards
-- wildcard cards. The mark of a card nobody planned: where it came from, who
-- proposed it, its pending dependencies and whether a person approved it (JSON).
ALTER TABLE tickets ADD COLUMN wildcard_json TEXT;
