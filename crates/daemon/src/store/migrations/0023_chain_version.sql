-- name: legacy-23-chain-version
-- the version a chain releases (the epic's release); none for the chains that exist.
ALTER TABLE chains ADD COLUMN version TEXT;
