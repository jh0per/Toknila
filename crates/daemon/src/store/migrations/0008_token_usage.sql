-- name: legacy-08-token-usage
-- token usage. Which Claude Code session a run is (from hook input), the
-- transcript files read so far (byte offset), one row per model call (the key is the
-- message id and request id, so a call is one row however often it was written), and
-- machine-wide settings (a key and a JSON value).
CREATE TABLE usage_sessions (
    session_id TEXT PRIMARY KEY,
    ticket_id TEXT NOT NULL REFERENCES tickets(id),
    run_id TEXT,
    role TEXT NOT NULL,
    transcript_path TEXT NOT NULL,
    at INTEGER NOT NULL
);
CREATE TABLE usage_files (
    path TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    session_id TEXT,
    read_offset INTEGER NOT NULL,
    workspace TEXT
);
CREATE INDEX usage_files_session ON usage_files(session_id);
CREATE TABLE usage_records (
    key TEXT PRIMARY KEY,
    file TEXT NOT NULL REFERENCES usage_files(path),
    ticket_id TEXT,
    at INTEGER NOT NULL,
    model TEXT NOT NULL,
    input INTEGER NOT NULL,
    cache_write INTEGER NOT NULL,
    cache_read INTEGER NOT NULL,
    output INTEGER NOT NULL
);
CREATE INDEX usage_records_at ON usage_records(at);
CREATE INDEX usage_records_ticket ON usage_records(ticket_id);
CREATE TABLE app_settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
