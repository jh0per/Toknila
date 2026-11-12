-- name: toknila-88-conversation
-- a project's conversation with its assistant: the person's messages, the assistant's `say`
-- and Toknila's lines about what the assistant did. Insert-only. A card's discussion stays in
-- the card's thread and is read into the conversation, so it is never stored here.
CREATE TABLE conversation_messages (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id),
    author TEXT NOT NULL,
    text TEXT NOT NULL,
    about_kind TEXT,
    about_ref TEXT,
    attachments_json TEXT NOT NULL DEFAULT '[]',
    created_at INTEGER NOT NULL
);
CREATE INDEX conversation_messages_project ON conversation_messages(project_id, created_at, id);
