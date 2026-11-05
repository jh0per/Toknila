use super::*;

const NOW: Timestamp = Timestamp(10);

fn thread() -> Thread {
    open(
        ThreadId::new(),
        TicketId::new(),
        1,
        ThreadKind::Design,
        " List screen ",
        Actor::Agent,
        NOW,
    )
    .expect("open")
}

fn artifact(t: &Thread, number: u32, body: &str) -> Result<ThreadVersion, ThreadError> {
    version(
        t,
        ThreadVersionId::new(),
        number,
        Actor::Agent,
        ArtifactFormat::Mermaid,
        body.to_owned(),
        NOW,
    )
}

fn approve(t: &Thread, by: Actor, latest: Option<u32>) -> Result<Thread, ThreadError> {
    apply(
        t,
        ThreadEvent::Approve {
            by,
            latest_version: latest,
        },
        Timestamp(20),
    )
    .map(|(t, _)| t)
}

#[test]
fn a_thread_opens_with_a_trimmed_title_and_needs_one() {
    let t = thread();
    assert_eq!(
        (t.title.as_str(), t.state, t.opened_by, t.approved_version),
        ("List screen", ThreadState::Open, Actor::Agent, None)
    );
    let e = open(
        ThreadId::new(),
        t.ticket_id,
        2,
        ThreadKind::Flow,
        "  ",
        Actor::Human,
        NOW,
    );
    assert_eq!(e, Err(ThreadError::EmptyTitle));
}

#[test]
fn approval_pins_the_latest_version_and_is_a_persons() {
    let t = thread();
    let (approved, change) = apply(
        &t,
        ThreadEvent::Approve {
            by: Actor::Human,
            latest_version: Some(2),
        },
        Timestamp(20),
    )
    .expect("approve");
    assert_eq!(
        (
            approved.state,
            approved.approved_version,
            approved.approved_at,
            change
        ),
        (
            ThreadState::Approved,
            Some(2),
            Some(Timestamp(20)),
            Some(ThreadChange::Approved)
        )
    );
    let silent = approve(&t, Actor::Human, None).expect("no artifact is fine");
    assert_eq!(
        (silent.state, silent.approved_version),
        (ThreadState::Approved, None)
    );
}

#[test]
fn an_agent_or_the_system_cannot_approve_or_request_changes() {
    let t = thread();
    for actor in [Actor::Agent, Actor::System] {
        assert_eq!(
            approve(&t, actor, Some(1)),
            Err(ThreadError::PersonOnly {
                action: "approve a thread",
                actor
            })
        );
        assert_eq!(
            apply(&t, ThreadEvent::ChangesRequested { by: actor }, NOW),
            Err(ThreadError::PersonOnly {
                action: "request changes",
                actor
            })
        );
    }
}

#[test]
fn approving_twice_is_refused() {
    let approved = approve(&thread(), Actor::Human, Some(1)).expect("approve");
    assert_eq!(
        approve(&approved, Actor::Human, Some(1)),
        Err(ThreadError::AlreadyApproved)
    );
}

#[test]
fn a_new_version_reopens_an_approved_thread_and_drops_the_pin() {
    let approved = approve(&thread(), Actor::Human, Some(1)).expect("approve");
    let (reopened, change) = apply(&approved, ThreadEvent::NewVersion, NOW).expect("version");
    assert_eq!(
        (
            reopened.state,
            reopened.approved_version,
            reopened.approved_at,
            change
        ),
        (ThreadState::Open, None, None, Some(ThreadChange::Reopened))
    );
    let (still, change) = apply(&reopened, ThreadEvent::NewVersion, NOW).expect("again");
    assert_eq!((still, change), (reopened, None));
}

#[test]
fn requesting_changes_keeps_or_puts_the_thread_open() {
    let open_thread = thread();
    let ask = ThreadEvent::ChangesRequested { by: Actor::Human };
    assert_eq!(
        apply(&open_thread, ask, NOW).expect("open stays"),
        (open_thread.clone(), None)
    );
    let approved = approve(&open_thread, Actor::Human, Some(1)).expect("approve");
    let (back, change) = apply(&approved, ask, NOW).expect("reopen");
    assert_eq!(
        (back.state, change),
        (ThreadState::Open, Some(ThreadChange::Reopened))
    );
}

#[test]
fn messages_need_text_and_leave_the_state_alone() {
    let t = thread();
    let m = message(
        &t,
        ThreadMessageId::new(),
        Actor::Human,
        MessageKind::Message,
        "  what about empty? ",
        NOW,
    )
    .expect("message");
    assert_eq!((m.text.as_str(), m.thread_id), ("what about empty?", t.id));
    let e = message(
        &t,
        ThreadMessageId::new(),
        Actor::Human,
        MessageKind::ChangesRequested,
        " ",
        NOW,
    );
    assert_eq!(e, Err(ThreadError::EmptyText("request for changes")));
}

#[test]
fn artifact_size_is_limited_to_512_kib() {
    let t = thread();
    let at_limit = "x".repeat(MAX_ARTIFACT_BYTES);
    assert!(artifact(&t, 1, &at_limit).is_ok());
    let over = "x".repeat(MAX_ARTIFACT_BYTES + 1);
    assert_eq!(
        artifact(&t, 1, &over),
        Err(ThreadError::ArtifactTooLarge {
            bytes: MAX_ARTIFACT_BYTES + 1,
            max: MAX_ARTIFACT_BYTES
        })
    );
    assert_eq!(artifact(&t, 1, "  \n"), Err(ThreadError::EmptyArtifact));
}

#[test]
fn hand_in_is_refused_while_a_thread_is_open_and_names_it() {
    use crate::machine::{TicketEvent, TransitionError, apply as step};
    use crate::{TicketDraft, TicketStatus};

    let mut running = crate::Ticket::new_backlog(
        TicketId::new(),
        crate::ChainId::new(),
        "DEMO-1".parse().expect("key"),
        TicketDraft::default(),
        NOW,
    );
    running.status = TicketStatus::Running;
    let hand_in = |threads: &[Thread]| {
        let event = TicketEvent::Finished {
            summary: "done".to_owned(),
            open_threads: open_threads(threads),
        };
        step(&running, &event, NOW).map(|t| t.ticket.status)
    };
    let first = thread();
    let mut second = thread();
    second.number = 2;
    second.title = "Flow".to_owned();
    second.kind = ThreadKind::Flow;
    assert_eq!(hand_in(&[]), Ok(TicketStatus::Review));
    let err = hand_in(&[first.clone(), second.clone()]).expect_err("open");
    assert_eq!(
        err.to_string(),
        "cannot hand in DEMO-1: 2 open: #1 \"List screen\" (Design), #2 \"Flow\" (Flow)"
    );
    let first = approve(&first, Actor::Human, None).expect("approve");
    let err = hand_in(&[first.clone(), second.clone()]).expect_err("one open");
    assert!(
        matches!(&err, TransitionError::OpenThreads { threads, .. } if threads.len() == 1 && threads[0].number == 2),
        "{err}"
    );
    let second = approve(&second, Actor::Human, None).expect("approve");
    assert_eq!(hand_in(&[first, second]), Ok(TicketStatus::Review));
}

#[test]
fn phase_is_design_while_any_thread_is_open() {
    let open_thread = thread();
    let approved = approve(&open_thread, Actor::Human, None).expect("approve");
    assert_eq!(phase(&[]), Phase::Implement);
    assert_eq!(phase(std::slice::from_ref(&approved)), Phase::Implement);
    assert_eq!(phase(&[approved, open_thread]), Phase::Design);
}
