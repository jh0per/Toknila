-- name: toknila-92-agent-paused
-- a Waiting card whose agent's process was ended: its session, when, and whether the answer came.
ALTER TABLE tickets ADD COLUMN agent_paused_json TEXT;
