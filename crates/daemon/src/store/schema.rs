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
    -- The worktree and branch of a card, and the process working in it.
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
    // 2: wildcard cards. The mark of a card nobody planned: where it came from, who
    // proposed it, its pending dependencies and whether a person approved it (JSON).
    r#"
    ALTER TABLE tickets ADD COLUMN wildcard_json TEXT;
    "#,
    // 3: threads on a card. Messages and versions are insert-only; a version number is
    // unique per thread, a thread number per ticket.
    r#"
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
    "#,
    // 4: cards imported as done behind open cards: the earlier cards that were open
    // (JSON), kept for good so a view can say "done out of order".
    r#"
    ALTER TABLE tickets ADD COLUMN done_out_of_order_json TEXT;
    "#,
    // 5: parallel agents. The project's limit of agents working at once, and the flag of a
    // Ready card a person started that waits for a free slot (its place in line is computed).
    r#"
    ALTER TABLE projects ADD COLUMN max_parallel INTEGER NOT NULL DEFAULT 2;
    ALTER TABLE tickets ADD COLUMN queued INTEGER NOT NULL DEFAULT 0;
    "#,
    // 6: review. A ticket's review state (whose turn, the rounds so far; JSON) and the
    // reviewer agent profile that may be set on a ticket, a chain and a project.
    r#"
    ALTER TABLE tickets ADD COLUMN review_json TEXT;
    ALTER TABLE tickets ADD COLUMN reviewer TEXT;
    ALTER TABLE chains ADD COLUMN reviewer TEXT;
    ALTER TABLE projects ADD COLUMN default_reviewer TEXT;
    "#,
    // 7: pull-request mode. A project's merge mode, pull request title template and merge
    // method, and the pull request of a card (JSON: number, url, checks).
    r#"
    ALTER TABLE projects ADD COLUMN merge_mode TEXT NOT NULL DEFAULT 'local';
    ALTER TABLE projects ADD COLUMN pr_title TEXT NOT NULL DEFAULT '{key}: {title}';
    ALTER TABLE projects ADD COLUMN merge_method TEXT NOT NULL DEFAULT 'merge';
    ALTER TABLE tickets ADD COLUMN pr_json TEXT;
    "#,
    // 8: token usage. Which Claude Code session a run is (from hook input), the
    // transcript files read so far (byte offset), one row per model call (the key is the
    // message id and request id, so a call is one row however often it was written), and
    // machine-wide settings (a key and a JSON value).
    r#"
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
    "#,
    // 9: the project's check command.
    r#"
    ALTER TABLE projects ADD COLUMN check_command TEXT;
    "#,
    // 10: archived projects. Hidden from the sidebar, data kept.
    r#"
    ALTER TABLE projects ADD COLUMN archived INTEGER NOT NULL DEFAULT 0;
    "#,
    // 11: where an imported chain came from: the spec folder or chain file, as an absolute path.
    r#"
    ALTER TABLE chains ADD COLUMN source TEXT;
    "#,
    // 12: the Claude model of a project's card agents, reviewers and planner.
    r#"
    ALTER TABLE projects ADD COLUMN worker_model TEXT;
    ALTER TABLE projects ADD COLUMN reviewer_model TEXT;
    ALTER TABLE projects ADD COLUMN planner_model TEXT;
    "#,
];
