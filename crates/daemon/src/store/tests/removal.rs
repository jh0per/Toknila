//! Deleting a project's rows: every table that hangs off it is emptied of them, the journal
//! entries about them go, and another project keeps everything.

use toknila_core::events::DomainEvent;
use toknila_core::usage::{Record, TokenCounts};
use toknila_core::{
    ArtifactFormat, MessageKind, Ship, Thread, ThreadId, ThreadKind, ThreadMessage,
    ThreadMessageId, ThreadState, ThreadVersion, ThreadVersionId,
};

use super::*;

/// Every table with rows of a project, and the column that ties the row to its ticket or chain.
const TABLES: [&str; 13] = [
    "projects",
    "chains",
    "tickets",
    "releases",
    "edges",
    "release_tickets",
    "subtasks",
    "comments",
    "threads",
    "thread_messages",
    "thread_versions",
    "ships",
    "usage_sessions",
];

/// A project `key` with one chain of two cards, a milestone, notes, a thread with a message and a
/// version, a ship, an agent session with one model call, and the journal entries about them.
fn fill(tx: &mut WriteTx<'_>, key: &str) -> Result<(), StoreError> {
    let mut p = project();
    p.key = key.parse().expect("key");
    let mut c = chain(p.id);
    c.name = format!("{key}-chain");
    let (mut a, mut b) = (ticket(c.id, 1), ticket(c.id, 2));
    a.key = format!("{key}-1").parse().expect("key");
    b.key = format!("{key}-2").parse().expect("key");
    let r = Release {
        id: ReleaseId::new(),
        chain_id: c.id,
        version: "0.1.0".to_owned(),
        channel: ReleaseChannel::TagOnly,
        start_mode: StartMode::Manual,
        state: ReleaseState::Planned,
        branch: None,
        cut_at: None,
        released_at: None,
        tag: None,
    };
    let view = ChainView {
        chain: c.clone(),
        edges: vec![Edge {
            chain_id: c.id,
            from: NodeRef::Ticket(a.id),
            to: NodeRef::Release(r.id),
            kind: EdgeKind::Normal,
        }],
        release_tickets: vec![(r.id, a.id)],
        tickets: vec![a.clone(), b],
        releases: vec![r.clone()],
    };
    let subtask = Subtask {
        id: SubtaskId::new(),
        ticket_id: a.id,
        position: 1,
        text: "test".into(),
        done: false,
        done_by: None,
    };
    let comment = Comment {
        id: CommentId::new(),
        ticket_id: a.id,
        author: Actor::Human,
        kind: CommentKind::Instruction,
        body: CommentBody::default(),
        created_at: NOW,
        delivered_at: None,
    };
    let thread = Thread {
        id: ThreadId::new(),
        ticket_id: a.id,
        number: 1,
        kind: ThreadKind::Design,
        title: "screen".into(),
        state: ThreadState::Open,
        opened_by: Actor::Human,
        created_at: NOW,
        approved_version: None,
        approved_at: None,
    };
    let message = ThreadMessage {
        id: ThreadMessageId::new(),
        thread_id: thread.id,
        author: Actor::Human,
        kind: MessageKind::Message,
        text: "states".into(),
        created_at: NOW,
    };
    let version = ThreadVersion {
        id: ThreadVersionId::new(),
        thread_id: thread.id,
        number: 1,
        author: Actor::Agent,
        format: ArtifactFormat::Markdown,
        body: "# screen".into(),
        created_at: NOW,
    };
    let ship = Ship {
        ticket_id: a.id,
        key: a.key.clone(),
        branch: format!("ship/{key}/1"),
        path: format!("/ships/{key}/1"),
    };
    let repo = tx.repo();
    repo.insert_project(&p)?;
    repo.insert_chain(&c)?;
    repo.save_chain_view(&view)?;
    repo.upsert_subtask(&subtask)?;
    repo.upsert_comment(&comment)?;
    repo.upsert_thread(&thread)?;
    repo.insert_thread_message(&message)?;
    repo.insert_thread_version(&version)?;
    repo.save_ship(&ship)?;
    let session = format!("session-{key}");
    let file = FileState {
        path: format!("/transcripts/{key}.jsonl"),
        offset: 10,
        workspace: None,
    };
    let call = Record {
        key: format!("call-{key}"),
        at: NOW,
        model: "m".into(),
        tokens: TokenCounts {
            output: 7,
            ..TokenCounts::default()
        },
    };
    repo.usage_ingest(&file, "claude", Some(&session), Some(a.id), &[call])?;
    repo.usage_session_set(&session, a.id, None, AgentRole::Worker, &file.path, NOW)?;
    for e in [
        DomainEvent::WindowShowCard {
            project: p.id,
            project_key: p.key.clone(),
            chain: c.name.clone(),
            ticket: a.key.clone(),
        },
        DomainEvent::ProjectCreated { project: p },
        DomainEvent::ChainCreated { chain: c },
        DomainEvent::TicketCreated { ticket: a.clone() },
        DomainEvent::ReleaseCreated { release: r },
        DomainEvent::SubtaskAdded {
            ticket: a.key.clone(),
            subtask,
        },
        DomainEvent::CommentAdded {
            ticket: a.key.to_string(),
            comment,
        },
        DomainEvent::ThreadOpened {
            ticket: a.key.clone(),
            thread,
        },
        DomainEvent::ShipCreated { ship },
    ] {
        tx.emit(&e)?;
    }
    Ok(())
}

async fn rows(store: &Store) -> Vec<(&'static str, i64)> {
    let mut out = Vec::new();
    for table in TABLES {
        out.push((table, count(store, table).await));
    }
    out
}

#[tokio::test]
async fn a_deleted_project_leaves_no_row_and_no_journal_entry_and_the_other_keeps_all() {
    let (_dir, store) = open_temp();
    store
        .write(Actor::Human, NOW, |tx| {
            fill(tx, "GONE")?;
            fill(tx, "KEPT")
        })
        .await
        .expect("fill");
    let full = rows(&store).await;
    assert_eq!(count(&store, "events").await, 18);
    let gone = store
        .read(|repo| repo.project_by_key("GONE"))
        .await
        .expect("read")
        .expect("project")
        .id;
    store
        .write(Actor::Human, NOW, move |tx| tx.repo().delete_project(gone))
        .await
        .expect("delete");
    let halved: Vec<(&str, i64)> = full.iter().map(|(t, n)| (*t, n / 2)).collect();
    assert_eq!(
        rows(&store).await,
        halved,
        "one project's rows of each table"
    );
    assert_eq!(
        count(&store, "events").await,
        9,
        "the kept project's entries, its window event among them"
    );
    let mentions = |key: &'static str| {
        let store = store.clone();
        async move {
            store
                .read(move |repo| {
                    Ok(repo.conn.query_row(
                        "SELECT COUNT(*) FROM events WHERE payload_json LIKE ?1",
                        [format!("%{key}%")],
                        |r| r.get::<_, i64>(0),
                    )?)
                })
                .await
                .expect("read")
        }
    };
    // Every entry but the milestone's names its project's key.
    assert_eq!((mentions("KEPT").await, mentions("GONE").await), (8, 0));
    // The tokens were spent: both calls stay, the deleted one tied to no card.
    let tied = store
        .read(|repo| {
            Ok(repo.conn.query_row(
                "SELECT COUNT(*) FROM usage_records WHERE ticket_id IS NOT NULL",
                [],
                |r| r.get::<_, i64>(0),
            )?)
        })
        .await
        .expect("read");
    assert_eq!((count(&store, "usage_records").await, tied), (2, 1));
}

#[tokio::test]
async fn a_deleted_chain_leaves_no_row_but_its_project_and_the_other_project_keeps_all() {
    let (_dir, store) = open_temp();
    store
        .write(Actor::Human, NOW, |tx| {
            fill(tx, "GONE")?;
            fill(tx, "KEPT")
        })
        .await
        .expect("fill");
    let full = rows(&store).await;
    let chain = store
        .read(|repo| repo.chains_named("GONE-chain"))
        .await
        .expect("read")[0]
        .id;
    store
        .write(Actor::Human, NOW, move |tx| tx.repo().delete_chain(chain))
        .await
        .expect("delete");
    let after = rows(&store).await;
    for ((table, before), (_, now)) in full.iter().zip(&after) {
        let expected = if *table == "projects" {
            *before
        } else {
            before / 2
        };
        assert_eq!(*now, expected, "{table}");
    }
    assert_eq!(
        count(&store, "events").await,
        11,
        "the project's own two entries (created, window) stay with the kept project's nine"
    );
}

#[tokio::test]
async fn a_deleted_project_takes_its_design_pins_and_their_journal_entries() {
    use toknila_core::design::Region;
    use toknila_core::design::pin::DesignPin;
    use toknila_core::{DesignId, PinId};

    let (_dir, store) = open_temp();
    let p = project();
    let id = p.id;
    let pin = DesignPin {
        id: PinId::new(),
        project: id,
        design: DesignId::new(),
        design_name: "home.png".to_owned(),
        number: 1,
        region: Region {
            x: 1,
            y: 2,
            w: 3,
            h: 4,
        },
        text: "Q?".to_owned(),
        card: None,
        answer: None,
        created_at: NOW,
        answered_at: None,
    };
    store
        .write(Actor::Human, NOW, move |tx| {
            tx.repo().insert_project(&p)?;
            tx.repo().upsert_design_pin(&pin)?;
            tx.emit(&DomainEvent::DesignPinAsked { pin })
        })
        .await
        .expect("fill");
    assert_eq!(count(&store, "design_pins").await, 1);
    assert_eq!(count(&store, "events").await, 1);
    store
        .write(Actor::Human, NOW, move |tx| tx.repo().delete_project(id))
        .await
        .expect("delete");
    assert_eq!(count(&store, "design_pins").await, 0);
    assert_eq!(count(&store, "events").await, 0);
}
