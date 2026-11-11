-- name: legacy-03-threads
-- threads on a card. Messages and versions are insert-only; a version number is
-- unique per thread, a thread number per ticket.
CREATE TABLE threads (
    id TEXT PRIMARY KEY,
    ticket_id TEXT NOT NULL REFERENCES tickets(id),
    number INTEGER NOT NULL,
    kind TEXT NOT NULL,
    title TEXT NOT NULL,
    state TEXT NOT NULL,
    opened_by TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    approved_version INTEGER,
    approved_at INTEGER,
    UNIQUE (ticket_id, number)
);
CREATE TABLE thread_messages (
    id TEXT PRIMARY KEY,
    thread_id TEXT NOT NULL REFERENCES threads(id),
    author TEXT NOT NULL,
    kind TEXT NOT NULL,
    text TEXT NOT NULL,
    created_at INTEGER NOT NULL
);
CREATE INDEX thread_messages_thread ON thread_messages(thread_id, created_at);
CREATE TABLE thread_versions (
    id TEXT PRIMARY KEY,
    thread_id TEXT NOT NULL REFERENCES threads(id),
    number INTEGER NOT NULL,
    author TEXT NOT NULL,
    format TEXT NOT NULL,
    body TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    UNIQUE (thread_id, number)
);
