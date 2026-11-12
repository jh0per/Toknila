-- name: toknila-87-roadmap-position
-- the order of a project's chains (its roadmap): from 0, kept from the order they were made in.
ALTER TABLE chains ADD COLUMN position INTEGER NOT NULL DEFAULT 0;
UPDATE chains SET position = (
  SELECT COUNT(*) FROM chains c2 WHERE c2.project_id = chains.project_id AND c2.rowid < chains.rowid
);
