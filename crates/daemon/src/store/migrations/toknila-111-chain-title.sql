-- name: toknila-111-chain-title
-- the words a person reads for an epic; NULL in rows older than titles (the name is shown made readable).
ALTER TABLE chains ADD COLUMN title TEXT;
