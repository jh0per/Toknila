-- name: legacy-11-chain-source
-- where an imported chain came from: the spec folder or chain file, as an absolute path.
ALTER TABLE chains ADD COLUMN source TEXT;
