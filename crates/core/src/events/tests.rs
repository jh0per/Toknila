use serde_json::{Value, json};

use super::*;
use crate::{ChainId, ReleaseChannel, ReleaseId, StartMode, TicketDraft, TicketId, Timestamp};

fn ticket(status: TicketStatus) -> Ticket {
    let mut t = Ticket::new_backlog(
        "01JBQ7ZJ0000000000000000T1"
            .parse::<TicketId>()
            .expect("id"),
        "01JBQ7ZJ0000000000000000C1".parse::<ChainId>().expect("id"),
        "DEMO-1".parse().expect("key"),
        TicketDraft {
            title: "A".into(),
            ..TicketDraft::default()
        },
        Timestamp(5),
    );
    t.status = status;
    t
}

fn payload(e: &DomainEvent) -> Value {
    serde_json::to_value(e).expect("serialize")
}

#[test]
fn status_change_carries_key_from_to_reason_and_ticket() {
    let before = ticket(TicketStatus::Ready);
    let after = ticket(TicketStatus::Running);
    let e = DomainEvent::ticket_changed(&before, &after);
    assert_eq!(
        (e.entity(), e.kind(), e.entity_id().as_str()),
        ("ticket", "status_changed", "01JBQ7ZJ0000000000000000T1")
    );
    assert_eq!(
        payload(&e),
        json!({
            "key": "DEMO-1",
            "from": "ready",
            "to": "running",
            "reason": null,
            "ticket": serde_json::to_value(&after).expect("ticket"),
        })
    );
}

#[test]
fn same_status_is_an_update() {
    let before = ticket(TicketStatus::Ready);
    let mut after = before.clone();
    after.title = "B".into();
    let e = DomainEvent::ticket_changed(&before, &after);
    assert_eq!((e.entity(), e.kind()), ("ticket", "updated"));
    assert_eq!(
        payload(&e),
        json!({ "ticket": serde_json::to_value(&after).expect("t") })
    );
}

#[test]
fn wildcard_decisions_name_the_card_and_carry_it() {
    let mut proposed = ticket(TicketStatus::Proposed);
    proposed.status_reason = Some(StatusReason::Proposed);
    let e = DomainEvent::TicketProposed {
        ticket: proposed.clone(),
    };
    assert_eq!(
        (e.entity(), e.kind(), e.entity_id().as_str()),
        ("ticket", "proposed", "01JBQ7ZJ0000000000000000T1")
    );
    assert_eq!(
        payload(&e),
        json!({ "ticket": serde_json::to_value(&proposed).expect("t") })
    );

    let approved = ticket(TicketStatus::Backlog);
    let e = DomainEvent::wildcard_decided(&proposed, &approved).expect("a decision");
    assert_eq!((e.entity(), e.kind()), ("ticket", "approved"));
    assert_eq!(
        payload(&e),
        json!({ "key": "DEMO-1", "ticket": serde_json::to_value(&approved).expect("t") })
    );

    let mut rejected = ticket(TicketStatus::Cancelled);
    rejected.status_reason = Some(StatusReason::Cancelled {
        text: "later".into(),
    });
    let e = DomainEvent::wildcard_decided(&proposed, &rejected).expect("a decision");
    assert_eq!((e.entity(), e.kind()), ("ticket", "rejected"));
    let p = payload(&e);
    assert_eq!(
        (&p["key"], &p["reason"]),
        (&json!("DEMO-1"), &json!("later"))
    );

    let ready = ticket(TicketStatus::Ready);
    assert_eq!(DomainEvent::wildcard_decided(&approved, &ready), None);
}

#[test]
fn release_change_names_version_and_states() {
    let before = Release::planned(
        "01JBQ7ZJ0000000000000000R1"
            .parse::<ReleaseId>()
            .expect("id"),
        "01JBQ7ZJ0000000000000000C1".parse::<ChainId>().expect("id"),
        "1.0".into(),
        ReleaseChannel::TagOnly,
        StartMode::Manual,
    );
    let mut after = before.clone();
    after.state = ReleaseState::Cut;
    let e = DomainEvent::release_changed(&before, &after);
    assert_eq!((e.entity(), e.kind()), ("release", "state_changed"));
    let p = payload(&e);
    assert_eq!(
        (&p["version"], &p["from"], &p["to"]),
        (&json!("1.0"), &json!("planned"), &json!("cut"))
    );
}

#[test]
fn edge_events_belong_to_their_chain() {
    let chain_id: ChainId = "01JBQ7ZJ0000000000000000C1".parse().expect("id");
    let edge = Edge {
        chain_id,
        from: crate::NodeRef::Ticket("01JBQ7ZJ0000000000000000T1".parse().expect("id")),
        to: crate::NodeRef::Ticket("01JBQ7ZJ0000000000000000T2".parse().expect("id")),
        kind: crate::EdgeKind::Normal,
    };
    let e = DomainEvent::EdgeAdded { edge };
    assert_eq!(
        (e.entity(), e.kind(), e.entity_id()),
        ("chain", "edge_added", chain_id.to_string())
    );
    assert_eq!(payload(&e)["edge"]["kind"], json!("normal"));
}

#[test]
fn thread_events_pin_their_names_and_payloads() {
    use crate::{Actor, ArtifactFormat, MessageKind, ThreadId, ThreadKind, ThreadMessageId};
    use crate::{ThreadMessage, thread};

    let id: ThreadId = "01JBQ7ZJ0000000000000000H1".parse().expect("id");
    let key: TicketKey = "DEMO-1".parse().expect("key");
    let t = thread::open(
        id,
        "01JBQ7ZJ0000000000000000T1".parse().expect("id"),
        1,
        ThreadKind::Design,
        "List screen",
        Actor::Agent,
        Timestamp(5),
    )
    .expect("open");
    let opened = DomainEvent::ThreadOpened {
        ticket: key.clone(),
        thread: t.clone(),
    };
    assert_eq!(
        (opened.entity(), opened.kind(), opened.entity_id().as_str()),
        ("thread", "opened", "01JBQ7ZJ0000000000000000H1")
    );
    assert_eq!(
        payload(&opened),
        json!({ "ticket": "DEMO-1", "thread": serde_json::to_value(&t).expect("t") })
    );

    let message = ThreadMessage {
        id: ThreadMessageId::new(),
        thread_id: id,
        author: Actor::Human,
        kind: MessageKind::ChangesRequested,
        text: "add an error state".into(),
        created_at: Timestamp(6),
    };
    let posted = DomainEvent::ThreadMessagePosted {
        ticket: key.clone(),
        message: message.clone(),
    };
    assert_eq!(
        (posted.entity_id().as_str(), posted.kind()),
        ("01JBQ7ZJ0000000000000000H1", "message_posted")
    );
    assert_eq!(
        payload(&posted),
        json!({ "ticket": "DEMO-1", "message": serde_json::to_value(&message).expect("m") })
    );

    let version = DomainEvent::ThreadVersionPosted {
        ticket: key.clone(),
        thread: id,
        number: 2,
        format: ArtifactFormat::Mermaid,
        author: Actor::Agent,
        bytes: 42,
    };
    assert_eq!(version.kind(), "version_posted");
    assert_eq!(
        payload(&version),
        json!({ "ticket": "DEMO-1", "thread": "01JBQ7ZJ0000000000000000H1", "number": 2,
                "format": "mermaid", "author": "agent", "bytes": 42 })
    );

    let approved = DomainEvent::ThreadApproved {
        ticket: key.clone(),
        thread: t.clone(),
    };
    let reopened = DomainEvent::ThreadReopened {
        ticket: key,
        thread: t,
    };
    assert_eq!(
        (approved.kind(), reopened.kind(), reopened.entity()),
        ("approved", "reopened", "thread")
    );
}

#[test]
fn ship_events_belong_to_the_ticket_and_carry_branch_and_path() {
    let ship = Ship {
        ticket_id: "01JBQ7ZJ0000000000000000T1"
            .parse::<TicketId>()
            .expect("id"),
        key: "DEMO-1".parse().expect("key"),
        branch: "ship/demo/demo-1-a".into(),
        path: "/h/ships/demo/demo/demo-1-a".into(),
    };
    let created = DomainEvent::ShipCreated { ship: ship.clone() };
    let removed = DomainEvent::ShipRemoved { ship };
    assert_eq!(
        (created.entity(), created.kind(), removed.kind()),
        ("ticket", "ship_created", "ship_removed")
    );
    assert_eq!(created.entity_id(), "01JBQ7ZJ0000000000000000T1");
    assert_eq!(
        payload(&removed),
        json!({ "ship": {
            "ticket_id": "01JBQ7ZJ0000000000000000T1",
            "key": "DEMO-1",
            "branch": "ship/demo/demo-1-a",
            "path": "/h/ships/demo/demo/demo-1-a",
        }})
    );
}

#[test]
fn runner_events_belong_to_the_ticket_and_carry_the_run() {
    use crate::{ProcessExit, Run, RunId};
    let run = Run {
        id: "01JBQ7ZJ0000000000000000R1".parse::<RunId>().expect("id"),
        ticket_id: "01JBQ7ZJ0000000000000000T1"
            .parse::<TicketId>()
            .expect("id"),
        key: "DEMO-1".parse().expect("key"),
        profile: "claude-code".into(),
        dir: "/h/ships/demo/demo/demo-1-a".into(),
        role: crate::AgentRole::Worker,
    };
    let started = DomainEvent::RunnerStarted { run: run.clone() };
    let exited = DomainEvent::RunnerExited {
        run,
        exit: ProcessExit {
            code: Some(101),
            signal: None,
        },
    };
    assert_eq!(
        (started.entity(), started.kind(), exited.kind()),
        ("ticket", "runner_started", "runner_exited")
    );
    assert_eq!(exited.entity_id(), "01JBQ7ZJ0000000000000000T1");
    assert_eq!(
        payload(&exited),
        json!({
            "run": {
                "id": "01JBQ7ZJ0000000000000000R1",
                "ticket_id": "01JBQ7ZJ0000000000000000T1",
                "key": "DEMO-1",
                "profile": "claude-code",
                "dir": "/h/ships/demo/demo/demo-1-a",
            },
            "exit": { "code": 101, "signal": null },
        })
    );
}

#[test]
fn question_events_carry_the_ticket_the_role_and_what_was_asked_or_answered() {
    use crate::AgentRole;
    use crate::question::{AnswerKind, QuestionKind};
    let key: TicketKey = "DEMO-1".parse().expect("key");
    let ticket: TicketId = "01JBQ7ZJ0000000000000000T1".parse().expect("id");
    let raised = DomainEvent::QuestionRaised {
        key: key.clone(),
        ticket,
        role: AgentRole::Reviewer,
        kind: QuestionKind::Permission,
    };
    let cleared = DomainEvent::QuestionCleared {
        key: key.clone(),
        ticket,
        role: AgentRole::Worker,
    };
    let answered = DomainEvent::QuestionAnswered {
        key,
        ticket,
        role: AgentRole::Reviewer,
        answer: AnswerKind::Deny,
    };
    for (e, kind) in [
        (&raised, "question_raised"),
        (&cleared, "question_cleared"),
        (&answered, "question_answered"),
    ] {
        assert_eq!((e.entity(), e.kind()), ("ticket", kind));
        assert_eq!(e.entity_id(), "01JBQ7ZJ0000000000000000T1");
    }
    assert_eq!(
        payload(&raised),
        json!({"key": "DEMO-1", "ticket": "01JBQ7ZJ0000000000000000T1", "role": "reviewer", "kind": "permission"})
    );
    assert_eq!(
        payload(&cleared),
        json!({"key": "DEMO-1", "ticket": "01JBQ7ZJ0000000000000000T1", "role": "worker"})
    );
    assert_eq!(
        payload(&answered),
        json!({"key": "DEMO-1", "ticket": "01JBQ7ZJ0000000000000000T1", "role": "reviewer", "answer": "deny"})
    );
}
