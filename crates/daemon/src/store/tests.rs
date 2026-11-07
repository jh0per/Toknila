use toknila_core::graph::ChainView;
use toknila_core::{
    Actor, Chain, ChainId, Comment, CommentBody, CommentId, CommentKind, Edge, EdgeKind, NodeRef,
    Project, ProjectId, Release, ReleaseChannel, ReleaseId, ReleaseState, ReviewMode, StartMode,
    StatusReason, Subtask, SubtaskId, Ticket, TicketId, TicketStatus, Timestamp, WaitingReason,
    Wildcard,
};

use super::*;

const NOW: Timestamp = Timestamp(42);

fn project() -> Project {
    Project {
        id: ProjectId::new(),
        key: "DEMO".parse().expect("key"),
        name: "Demo".to_owned(),
        repo_path: None,
        work_branch: "develop".to_owned(),
        release_branch: "main".to_owned(),
        default_profile: "sim".to_owned(),
        default_review_mode: ReviewMode::Human,
        default_reviewer: None,
        max_parallel: 2,
        merge: Default::default(),
        check_command: None,
        archived: false,
    }
}

fn chain(project: ProjectId) -> Chain {
    Chain {
        id: ChainId::new(),
        project_id: project,
        name: "demo".to_owned(),
        goal: "gate".to_owned(),
        autostart: true,
        review_mode: Some(ReviewMode::Both),
        reviewer: None,
    }
}

fn ticket(chain: ChainId, n: u32) -> Ticket {
    Ticket {
        id: TicketId::new(),
        chain_id: chain,
        key: format!("DEMO-{n}").parse().expect("key"),
        title: format!("ticket {n}"),
        prompt: "do it".to_owned(),
        acceptance: vec!["green".to_owned()],
        status: TicketStatus::Backlog,
        status_reason: None,
        status_since: Timestamp(1),
        paused: false,
        queued: false,
        profile: Some("sim".to_owned()),
        review_mode: None,
        reviewer: None,
        branch: None,
        needs_secrets: false,
        tokens_used: 0,
        wildcard: None,
        done_out_of_order: None,
        pr: None,
        review: toknila_core::Review::default(),
    }
}

/// Writes a project and a chain the way the first migration's columns hold them, as a database
/// from before later columns existed has them.
fn insert_phase_0_rows(conn: &Connection, p: &Project, c: &Chain) {
    conn.execute(
        "INSERT INTO projects (id, key, name, repo_path, work_branch, release_branch, \
         default_profile, default_review_mode) VALUES (?1, 'DEMO', 'Demo', NULL, 'develop', 'main', \
         'sim', 'human')",
        params![p.id.to_string()],
    )
    .expect("phase-0 project");
    conn.execute(
        "INSERT INTO chains (id, project_id, name, goal, autostart, review_mode) \
         VALUES (?1, ?2, 'demo', 'gate', 1, 'both')",
        params![c.id.to_string(), p.id.to_string()],
    )
    .expect("phase-0 chain");
}

fn open_temp() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = Store::open(&dir.path().join("toknila.db")).expect("open");
    (dir, store)
}

async fn count(store: &Store, table: &'static str) -> i64 {
    store
        .read(move |repo| {
            Ok(repo
                .conn
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))?)
        })
        .await
        .expect("count")
}

#[tokio::test]
async fn migrations_from_zero_and_reopen_are_idempotent() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("toknila.db");
    for _ in 0..3 {
        let store = Store::open(&path).expect("open");
        let version: i64 = store
            .read(|repo| {
                Ok(repo
                    .conn
                    .pragma_query_value(None, "user_version", |r| r.get(0))?)
            })
            .await
            .expect("version");
        assert_eq!(version, SCHEMA_VERSION);
        assert_eq!(
            count(&store, "agent_profiles").await,
            1,
            "seed rows are not duplicated"
        );
        drop(store);
    }
}

#[tokio::test]
async fn a_phase_0_database_gains_wildcard_marks() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("toknila.db");
    // A database written by phase 0: the first migration only, one ticket.
    let p = project();
    let c = chain(p.id);
    let old = ticket(c.id, 1);
    {
        let conn = Connection::open(&path).expect("raw open");
        conn.execute_batch(schema::MIGRATIONS[0])
            .expect("phase-0 schema");
        conn.pragma_update(None, "user_version", 1)
            .expect("version");
        insert_phase_0_rows(&conn, &p, &c);
        conn.execute(
            "INSERT INTO tickets (id, chain_id, key, title, prompt, acceptance_json, status, \
             status_since, paused, needs_secrets, tokens_used) \
             VALUES (?1, ?2, 'DEMO-1', ?3, ?4, '[\"green\"]', 'backlog', 1, 0, 0, 0)",
            params![old.id.to_string(), c.id.to_string(), old.title, old.prompt],
        )
        .expect("phase-0 ticket");
    }
    let store = Store::open(&path).expect("migrates");
    let id = old.id;
    let pid = p.id;
    let migrated = store
        .read(move |repo| repo.project(pid))
        .await
        .expect("read");
    assert_eq!(migrated.map(|p| p.max_parallel), Some(2));
    let loaded = store.read(move |repo| repo.ticket(id)).await.expect("read");
    assert_eq!(loaded.as_ref().map(|t| t.queued), Some(false));
    assert_eq!(loaded.as_ref().map(|t| &t.wildcard), Some(&None));
    assert_eq!(loaded.map(|t| t.title), Some(old.title.clone()));

    let mut card = ticket(c.id, 2);
    card.status = TicketStatus::Proposed;
    card.status_reason = Some(StatusReason::Proposed);
    card.wildcard = Some(Wildcard {
        from: Some("DEMO-1".parse().expect("key")),
        proposed_by: Actor::Agent,
        after: vec![NodeRef::Ticket(id)],
        before: vec![],
        approved: false,
    });
    let saved = card.clone();
    store
        .write(Actor::Agent, NOW, move |tx| tx.repo().upsert_ticket(&saved))
        .await
        .expect("save card");
    let card_id = card.id;
    let back = store
        .read(move |repo| repo.ticket(card_id))
        .await
        .expect("read card");
    assert_eq!(back, Some(card));
}

#[tokio::test]
async fn the_out_of_order_mark_is_stored_and_old_rows_have_none() {
    let (_dir, store) = open_temp();
    let p = project();
    let c = chain(p.id);
    let (pc, cc) = (p.clone(), c.clone());
    store
        .write(Actor::Human, NOW, move |tx| {
            tx.repo().insert_project(&pc)?;
            tx.repo().insert_chain(&cc)
        })
        .await
        .expect("setup");
    let plain = ticket(c.id, 1);
    let mut marked = ticket(c.id, 2);
    marked.status = TicketStatus::Merged;
    marked.done_out_of_order = Some(toknila_core::OutOfOrder::new(vec![plain.key.clone()]));
    let (a, b) = (plain.clone(), marked.clone());
    store
        .write(Actor::Human, NOW, move |tx| {
            tx.repo().upsert_ticket(&a)?;
            tx.repo().upsert_ticket(&b)
        })
        .await
        .expect("save");
    let (ia, ib) = (plain.id, marked.id);
    let back = store
        .read(move |repo| Ok((repo.ticket(ia)?, repo.ticket(ib)?)))
        .await
        .expect("read");
    assert_eq!(back, (Some(plain), Some(marked)));
}

#[tokio::test]
async fn review_state_and_reviewer_settings_are_stored_and_old_rows_have_none() {
    use toknila_core::{ReviewTurn, Verdict};

    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("toknila.db");
    let p = project();
    let c = chain(p.id);
    let old = ticket(c.id, 1);
    {
        // A database from before review: the first five migrations, one ticket in Review.
        let conn = Connection::open(&path).expect("raw open");
        for m in &schema::MIGRATIONS[..5] {
            conn.execute_batch(m).expect("schema");
        }
        conn.pragma_update(None, "user_version", 5)
            .expect("version");
        insert_phase_0_rows(&conn, &p, &c);
        conn.execute(
            "INSERT INTO tickets (id, chain_id, key, title, prompt, acceptance_json, status, \
             status_since, paused, needs_secrets, tokens_used) \
             VALUES (?1, ?2, 'DEMO-1', ?3, ?4, '[]', 'review', 1, 0, 0, 0)",
            params![old.id.to_string(), c.id.to_string(), old.title, old.prompt],
        )
        .expect("old ticket");
    }
    let store = Store::open(&path).expect("migrates");
    let id = old.id;
    let loaded = store
        .read(move |repo| repo.ticket(id))
        .await
        .expect("read")
        .expect("ticket");
    assert!(loaded.review.is_empty());
    assert_eq!(loaded.reviewer, None);

    let mut reviewed = ticket(c.id, 2);
    reviewed.status = TicketStatus::Review;
    reviewed.reviewer = Some("strict".to_owned());
    reviewed.review.turn = Some(ReviewTurn::Agent { decides: false });
    reviewed
        .review
        .record(Actor::Agent, Verdict::RequestChanges, "rename it", NOW);
    let (mut p2, mut c2) = (p.clone(), c.clone());
    p2.default_reviewer = Some("p-reviewer".to_owned());
    c2.reviewer = Some("c-reviewer".to_owned());
    let (saved, pp, cc) = (reviewed.clone(), p2.clone(), c2.clone());
    store
        .write(Actor::Human, NOW, move |tx| {
            tx.repo().update_project(&pp)?;
            tx.repo().update_chain(&cc)?;
            tx.repo().upsert_ticket(&saved)
        })
        .await
        .expect("save");
    let (tid, pid, cid) = (reviewed.id, p.id, c.id);
    let back = store
        .read(move |repo| Ok((repo.ticket(tid)?, repo.project(pid)?, repo.chain(cid)?)))
        .await
        .expect("read");
    assert_eq!(back, (Some(reviewed), Some(p2), Some(c2)));
}

#[tokio::test]
async fn a_database_before_threads_gains_them_and_keeps_every_byte_of_an_artifact() {
    use toknila_core::{
        ArtifactFormat, MessageKind, ThreadId, ThreadKind, ThreadMessageId, ThreadVersionId, thread,
    };

    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("toknila.db");
    let p = project();
    let c = chain(p.id);
    let t = ticket(c.id, 1);
    {
        let conn = Connection::open(&path).expect("raw open");
        conn.execute_batch(schema::MIGRATIONS[0]).expect("schema 1");
        conn.execute_batch(schema::MIGRATIONS[1]).expect("schema 2");
        conn.pragma_update(None, "user_version", 2)
            .expect("version");
        insert_phase_0_rows(&conn, &p, &c);
        conn.execute(
            "INSERT INTO tickets (id, chain_id, key, title, prompt, acceptance_json, status, \
             status_since, paused, needs_secrets, tokens_used) \
             VALUES (?1, ?2, 'DEMO-1', ?3, ?4, '[]', 'backlog', 1, 0, 0, 0)",
            params![t.id.to_string(), c.id.to_string(), t.title, t.prompt],
        )
        .expect("ticket");
    }
    let store = Store::open(&path).expect("migrates");
    let th = thread::open(
        ThreadId::new(),
        t.id,
        1,
        ThreadKind::Design,
        "List",
        Actor::Agent,
        NOW,
    )
    .expect("thread");
    let body = "ключ ✓ \"quoted\"\n<b>html</b>".to_owned();
    let v = thread::version(
        &th,
        ThreadVersionId::new(),
        1,
        Actor::Agent,
        ArtifactFormat::Html,
        body,
        NOW,
    )
    .expect("version");
    let m = thread::message(
        &th,
        ThreadMessageId::new(),
        Actor::Human,
        MessageKind::ChangesRequested,
        "more",
        NOW,
    )
    .expect("message");
    let (th2, v2, m2) = (th.clone(), v.clone(), m.clone());
    store
        .write(Actor::Agent, NOW, move |tx| -> Result<(), StoreError> {
            let repo = tx.repo();
            repo.upsert_thread(&th2)?;
            repo.insert_thread_version(&v2)?;
            repo.insert_thread_message(&m2)?;
            Ok(())
        })
        .await
        .expect("write");
    let tid = t.id;
    let (threads, versions, messages) = store
        .read(move |repo| {
            let threads = repo.threads(tid)?;
            let id = threads[0].id;
            Ok((
                threads,
                repo.thread_versions(id)?,
                repo.thread_messages(id)?,
            ))
        })
        .await
        .expect("read");
    assert_eq!(
        (threads, versions, messages),
        (vec![th.clone()], vec![v.clone()], vec![m])
    );

    // A version number and a thread number are unique; a second insert is refused.
    let again = store
        .write(Actor::Agent, NOW, move |tx| -> Result<(), StoreError> {
            let dup = toknila_core::ThreadVersion {
                id: ThreadVersionId::new(),
                ..v
            };
            tx.repo().insert_thread_version(&dup)
        })
        .await;
    assert!(again.is_err(), "version 1 already exists");
}

#[tokio::test]
async fn newer_database_is_refused() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("toknila.db");
    drop(Store::open(&path).expect("open"));
    let conn = Connection::open(&path).expect("raw open");
    conn.pragma_update(None, "user_version", SCHEMA_VERSION + 1)
        .expect("bump");
    drop(conn);
    assert!(matches!(Store::open(&path), Err(StoreError::TooNew { .. })));
}

#[tokio::test]
async fn write_commits_state_and_events_together() {
    let (_dir, store) = open_temp();
    let p = project();
    let mut live = store.subscribe();
    let ((), events) = store
        .write(Actor::Human, NOW, move |tx| -> Result<(), StoreError> {
            tx.repo().insert_project(&p)?;
            tx.emit(&toknila_core::events::DomainEvent::ProjectCreated { project: p.clone() })?;
            Ok(())
        })
        .await
        .expect("write");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].seq, 1);
    assert_eq!(events[0].at, NOW);
    assert_eq!(live.try_recv().expect("published").seq, 1);
    assert_eq!(count(&store, "projects").await, 1);
    assert_eq!(store.events_since(0, 100).await.expect("events"), events);
}

#[tokio::test]
async fn failed_command_leaves_no_half_changes() {
    let (_dir, store) = open_temp();
    let mut live = store.subscribe();
    let p = project();
    let err = store
        .write(Actor::Human, NOW, move |tx| -> Result<(), StoreError> {
            tx.repo().insert_project(&p)?;
            tx.emit(&toknila_core::events::DomainEvent::ProjectCreated { project: p.clone() })?;
            Err(StoreError::Corrupt("refused halfway".into()))
        })
        .await;
    assert!(err.is_err());
    assert_eq!(count(&store, "projects").await, 0);
    assert_eq!(count(&store, "events").await, 0);
    assert!(
        live.try_recv().is_err(),
        "nothing published for a rolled-back command"
    );
}

#[tokio::test]
async fn panicking_command_rolls_back_and_store_keeps_working() {
    let (_dir, store) = open_temp();
    let p = project();
    let err = store
        .write(Actor::Human, NOW, move |tx| -> Result<(), StoreError> {
            tx.repo().insert_project(&p)?;
            panic!("boom");
        })
        .await;
    assert!(
        matches!(err, Err(StoreError::Panicked(ref m)) if m == "boom"),
        "{err:?}"
    );
    assert_eq!(count(&store, "projects").await, 0);
    let p = project();
    store
        .write(Actor::Human, NOW, move |tx| tx.repo().insert_project(&p))
        .await
        .expect("next write");
    assert_eq!(count(&store, "projects").await, 1);
}

#[tokio::test]
async fn event_seq_is_increasing_and_resumable() {
    let (_dir, store) = open_temp();
    for i in 0..5 {
        store
            .write(Actor::System, Timestamp(i), move |tx| {
                tx.append("test", i, "tick", serde_json::json!({ "i": i }))
            })
            .await
            .expect("write");
    }
    let tail = store.events_since(2, 100).await.expect("events");
    assert_eq!(
        tail.iter().map(|e| e.seq).collect::<Vec<_>>(),
        vec![3, 4, 5]
    );
    assert_eq!(tail[0].payload, serde_json::json!({ "i": 2 }));
    assert_eq!(store.events_since(0, 2).await.expect("events").len(), 2);
}

#[tokio::test]
async fn ticket_numbers_are_reserved_per_project() {
    let (_dir, store) = open_temp();
    let p = project();
    let id = p.id;
    let (numbers, _) = store
        .write(Actor::Human, NOW, move |tx| {
            tx.repo().insert_project(&p)?;
            (0..3)
                .map(|_| tx.repo().take_ticket_number(id))
                .collect::<Result<Vec<_>, _>>()
        })
        .await
        .expect("write");
    assert_eq!(numbers, vec![1, 2, 3]);
}

#[tokio::test]
async fn chain_view_saves_and_loads_without_loss() {
    let (_dir, store) = open_temp();
    let p = project();
    let c = chain(p.id);
    let mut a = ticket(c.id, 1);
    a.status = TicketStatus::Merged;
    let mut b = ticket(c.id, 2);
    b.status = TicketStatus::Waiting;
    b.status_reason = Some(StatusReason::Waiting(WaitingReason::Question {
        text: "?".into(),
    }));
    b.tokens_used = 99;
    let r = Release {
        id: ReleaseId::new(),
        chain_id: c.id,
        version: "0.1.0".to_owned(),
        channel: ReleaseChannel::TagOnly,
        start_mode: StartMode::Manual,
        state: ReleaseState::Cut,
        branch: None,
        cut_at: Some(Timestamp(7)),
        released_at: None,
    };
    let view = ChainView {
        chain: c.clone(),
        edges: vec![
            Edge {
                chain_id: c.id,
                from: NodeRef::Ticket(a.id),
                to: NodeRef::Ticket(b.id),
                kind: EdgeKind::Normal,
            },
            Edge {
                chain_id: c.id,
                from: NodeRef::Ticket(a.id),
                to: NodeRef::Release(r.id),
                kind: EdgeKind::Normal,
            },
            Edge {
                chain_id: c.id,
                from: NodeRef::Release(r.id),
                to: NodeRef::Ticket(b.id),
                kind: EdgeKind::AfterRelease,
            },
        ],
        release_tickets: vec![(r.id, a.id)],
        tickets: vec![a, b],
        releases: vec![r],
    };
    let saved = view.clone();
    let chain_id = c.id;
    store
        .write(Actor::Human, NOW, move |tx| {
            tx.repo().insert_project(&p)?;
            tx.repo().insert_chain(&c)?;
            tx.repo().save_chain_view(&saved)
        })
        .await
        .expect("save");
    let loaded = store
        .read(move |repo| repo.chain_view(chain_id))
        .await
        .expect("load");
    assert_eq!(loaded, Some(view.clone()));

    // Saving again (an update) does not duplicate edges or memberships.
    let again = view.clone();
    store
        .write(Actor::Human, NOW, move |tx| {
            tx.repo().save_chain_view(&again)
        })
        .await
        .expect("resave");
    assert_eq!(
        store
            .read(move |repo| repo.chain_view(chain_id))
            .await
            .expect("load"),
        Some(view)
    );
}

#[tokio::test]
async fn subtasks_and_comments_roundtrip() {
    let (_dir, store) = open_temp();
    let p = project();
    let c = chain(p.id);
    let t = ticket(c.id, 1);
    let tid = t.id;
    let s = Subtask {
        id: SubtaskId::new(),
        ticket_id: tid,
        position: 1,
        text: "test".into(),
        done: true,
        done_by: Some(Actor::Agent),
    };
    let cm = Comment {
        id: CommentId::new(),
        ticket_id: tid,
        author: Actor::Human,
        kind: CommentKind::Instruction,
        body: CommentBody {
            gist: "Solid".into(),
            ..CommentBody::default()
        },
        created_at: Timestamp(5),
        delivered_at: None,
    };
    let (s2, cm2) = (s.clone(), cm.clone());
    store
        .write(Actor::Human, NOW, move |tx| {
            let repo = tx.repo();
            repo.insert_project(&p)?;
            repo.insert_chain(&c)?;
            repo.upsert_ticket(&t)?;
            repo.upsert_subtask(&s2)?;
            repo.upsert_comment(&cm2)
        })
        .await
        .expect("write");
    let (subs, comments) = store
        .read(move |repo| Ok((repo.subtasks(tid)?, repo.comments(tid)?)))
        .await
        .expect("read");
    assert_eq!(subs, vec![s]);
    assert_eq!(comments, vec![cm]);
}
