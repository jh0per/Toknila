use toknila_core::graph::ChainView;
use toknila_core::{
    Actor, Chain, ChainId, Comment, CommentBody, CommentId, CommentKind, Edge, EdgeKind, NodeRef,
    Project, ProjectId, Release, ReleaseChannel, ReleaseId, ReleaseState, ReviewMode, StartMode,
    StatusReason, Subtask, SubtaskId, Ticket, TicketId, TicketStatus, Timestamp, WaitingReason,
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
        profile: Some("sim".to_owned()),
        review_mode: None,
        branch: None,
        needs_secrets: false,
        tokens_used: 0,
    }
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
            tx.emit("project", p.id, "created", &p)?;
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
            tx.emit("project", p.id, "created", &p)?;
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
                tx.emit("test", i, "tick", serde_json::json!({ "i": i }))
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
