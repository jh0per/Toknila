//! Schema migrations. Each entry runs once, in order, inside one transaction, and bumps
//! `PRAGMA user_version`. Never edit a released migration; append a new one.

pub(super) const MIGRATIONS: &[&str] = &[
    // 1: phase-0 schema.
    r#"
    CREATE TABLE projects (
        id TEXT PRIMARY KEY,
        key TEXT NOT NULL UNIQUE,
        name TEXT NOT NULL,
        repo_path TEXT,
        work_branch TEXT NOT NULL,
        release_branch TEXT NOT NULL,
        default_profile TEXT NOT NULL,
        default_review_mode TEXT NOT NULL,
        next_ticket_number INTEGER NOT NULL DEFAULT 1
    );
    CREATE TABLE chains (
        id TEXT PRIMARY KEY,
        project_id TEXT NOT NULL REFERENCES projects(id),
        name TEXT NOT NULL,
        goal TEXT NOT NULL,
        autostart INTEGER NOT NULL,
        review_mode TEXT,
        UNIQUE (project_id, name)
    );
    CREATE TABLE tickets (
        id TEXT PRIMARY KEY,
        chain_id TEXT NOT NULL REFERENCES chains(id),
        key TEXT NOT NULL UNIQUE,
        title TEXT NOT NULL,
        prompt TEXT NOT NULL,
        acceptance_json TEXT NOT NULL,
        status TEXT NOT NULL,
        status_reason_json TEXT,
        status_since INTEGER NOT NULL,
        paused INTEGER NOT NULL,
        profile TEXT,
        review_mode TEXT,
        branch TEXT,
        needs_secrets INTEGER NOT NULL,
        tokens_used INTEGER NOT NULL
    );
    CREATE INDEX tickets_chain ON tickets(chain_id);
    CREATE TABLE releases (
        id TEXT PRIMARY KEY,
        chain_id TEXT NOT NULL REFERENCES chains(id),
        version TEXT NOT NULL,
        channel TEXT NOT NULL,
        start_mode TEXT NOT NULL,
        state TEXT NOT NULL,
        branch TEXT,
        cut_at INTEGER,
        released_at INTEGER,
        UNIQUE (chain_id, version)
    );
    CREATE TABLE edges (
        chain_id TEXT NOT NULL REFERENCES chains(id),
        from_kind TEXT NOT NULL,
        from_id TEXT NOT NULL,
        to_kind TEXT NOT NULL,
        to_id TEXT NOT NULL,
        kind TEXT NOT NULL,
        PRIMARY KEY (chain_id, from_id, to_id)
    );
    CREATE TABLE release_tickets (
        release_id TEXT NOT NULL REFERENCES releases(id),
        ticket_id TEXT NOT NULL REFERENCES tickets(id),
        PRIMARY KEY (release_id, ticket_id)
    );
    CREATE TABLE subtasks (
        id TEXT PRIMARY KEY,
        ticket_id TEXT NOT NULL REFERENCES tickets(id),
        position INTEGER NOT NULL,
        text TEXT NOT NULL,
        done INTEGER NOT NULL,
        done_by TEXT
    );
    CREATE INDEX subtasks_ticket ON subtasks(ticket_id, position);
    CREATE TABLE comments (
        id TEXT PRIMARY KEY,
        ticket_id TEXT NOT NULL REFERENCES tickets(id),
        author_kind TEXT NOT NULL,
        kind TEXT NOT NULL,
        body_json TEXT NOT NULL,
        created_at INTEGER NOT NULL,
        delivered_at INTEGER
    );
    CREATE INDEX comments_ticket ON comments(ticket_id, created_at);
    CREATE TABLE agent_profiles (
        name TEXT PRIMARY KEY,
        role TEXT NOT NULL,
        config_json TEXT NOT NULL
    );
    INSERT INTO agent_profiles (name, role, config_json) VALUES ('sim', 'worker', '{"kind":"sim"}');
    -- Phase 1 fills these.
    CREATE TABLE ships (
        id TEXT PRIMARY KEY,
        ticket_id TEXT NOT NULL REFERENCES tickets(id),
        worktree_path TEXT NOT NULL,
        branch TEXT NOT NULL,
        state TEXT NOT NULL
    );
    CREATE TABLE runners (
        id TEXT PRIMARY KEY,
        ship_id TEXT NOT NULL REFERENCES ships(id),
        profile TEXT NOT NULL,
        state TEXT NOT NULL,
        started_at INTEGER NOT NULL,
        nudges INTEGER NOT NULL DEFAULT 0,
        tokens_used INTEGER NOT NULL DEFAULT 0
    );
    CREATE TABLE events (
        seq INTEGER PRIMARY KEY AUTOINCREMENT,
        at INTEGER NOT NULL,
        actor TEXT NOT NULL,
        entity TEXT NOT NULL,
        entity_id TEXT NOT NULL,
        kind TEXT NOT NULL,
        payload_json TEXT NOT NULL
    );
    CREATE INDEX events_entity ON events(entity, entity_id, seq);
    "#,
    // 2: wildcard cards (P1-01). The mark of a card nobody planned: where it came from, who
    // proposed it, its pending dependencies and whether a person approved it (JSON).
    r#"
    ALTER TABLE tickets ADD COLUMN wildcard_json TEXT;
    "#,
];
