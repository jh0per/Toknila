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
            open_threads: open_threads(threads, |_| true),
            mode: crate::ReviewMode::Human,
            check: false,
            max_rounds: 3,
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
        matches!(&err, TransitionError::OpenThreads(_, threads) if threads.len() == 1 && threads[0].number == 2),
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

fn ask(at: i64) -> ThreadMessage {
    let t = thread();
    let kind = MessageKind::ChangesRequested;
    message(
        &t,
        ThreadMessageId::new(),
        Actor::Human,
        kind,
        "fix",
        Timestamp(at),
    )
    .expect("msg")
}

fn plain(at: i64) -> ThreadMessage {
    let t = thread();
    let kind = MessageKind::Message;
    message(
        &t,
        ThreadMessageId::new(),
        Actor::Human,
        kind,
        "hi",
        Timestamp(at),
    )
    .expect("msg")
}

fn version_at(at: i64) -> ThreadVersion {
    let mut v = artifact(&thread(), 1, "graph TD").expect("version");
    v.created_at = Timestamp(at);
    v
}

#[test]
fn changes_requested_is_false_without_messages() {
    assert!(!changes_requested(&thread(), &[], &[]));
    assert!(!changes_requested(&thread(), &[version_at(5)], &[plain(6)]));
}

#[test]
fn changes_requested_holds_until_a_newer_version_answers() {
    let asked = [plain(1), ask(5)];
    assert!(changes_requested(&thread(), &[version_at(2)], &asked));
    assert!(changes_requested(&thread(), &[], &asked));
    assert!(!changes_requested(
        &thread(),
        &[version_at(2), version_at(9)],
        &asked
    ));
    // A version posted at the same instant does not answer it.
    assert!(changes_requested(&thread(), &[version_at(5)], &asked));
}

#[test]
fn changes_requested_is_false_once_approved() {
    let t = thread();
    let approved = approve(&t, Actor::Human, Some(1)).expect("approve");
    assert!(!changes_requested(&approved, &[version_at(2)], &[ask(5)]));
}

#[test]
fn the_newest_request_decides() {
    let msgs = [ask(3), ask(8)];
    assert!(changes_requested(&thread(), &[version_at(5)], &msgs));
}

fn withdraw(
    t: &Thread,
    by: Actor,
    versions: usize,
    person_messages: usize,
) -> Result<Thread, ThreadError> {
    let event = ThreadEvent::Withdraw {
        by,
        versions,
        person_messages,
    };
    apply(t, event, Timestamp(20)).map(|(t, change)| {
        assert_eq!(change, Some(ThreadChange::Withdrawn));
        t
    })
}

#[test]
fn an_agent_withdraws_its_own_empty_thread() {
    let t = withdraw(&thread(), Actor::Agent, 0, 0).expect("withdraw");
    assert_eq!(t.state, ThreadState::Withdrawn);
    assert_eq!(phase(&[t]), Phase::Implement);
}

#[test]
fn a_thread_with_a_version_or_a_person_message_is_not_withdrawn() {
    assert_eq!(
        withdraw(&thread(), Actor::Agent, 1, 0),
        Err(ThreadError::NotEmpty { number: 1 })
    );
    assert_eq!(
        withdraw(&thread(), Actor::Agent, 0, 1),
        Err(ThreadError::NotEmpty { number: 1 })
    );
}

#[test]
fn only_the_agent_that_opened_a_thread_withdraws_it() {
    assert_eq!(
        withdraw(&thread(), Actor::Human, 0, 0),
        Err(ThreadError::NotYours)
    );
    let mut by_person = thread();
    by_person.opened_by = Actor::Human;
    assert_eq!(
        withdraw(&by_person, Actor::Agent, 0, 0),
        Err(ThreadError::NotYours)
    );
    let done = approve(&thread(), Actor::Human, None).expect("approve");
    assert_eq!(
        withdraw(&done, Actor::Agent, 0, 0),
        Err(ThreadError::NotOpen)
    );
}

#[test]
fn an_empty_thread_never_blocks_the_hand_in() {
    let t = thread();
    assert!(open_threads(std::slice::from_ref(&t), |_| false).is_empty());
    let blocking = open_threads(std::slice::from_ref(&t), |_| true);
    assert_eq!(blocking.len(), 1);
    assert_eq!(blocking[0].number, 1);
}

#[test]
fn a_second_thread_of_a_kind_names_the_first_unless_it_was_withdrawn() {
    let t = thread();
    let found = existing_of_kind(std::slice::from_ref(&t), ThreadKind::Design).map(|x| x.number);
    assert_eq!(found, Some(1));
    assert!(existing_of_kind(std::slice::from_ref(&t), ThreadKind::Flow).is_none());
    let done = approve(&t, Actor::Human, None).expect("approve");
    assert!(existing_of_kind(std::slice::from_ref(&done), ThreadKind::Design).is_some());
    let gone = withdraw(&t, Actor::Agent, 0, 0).expect("withdraw");
    assert!(existing_of_kind(&[gone], ThreadKind::Design).is_none());
    let err = ThreadError::DuplicateKind {
        kind: ThreadKind::Design,
        number: 1,
    };
    assert!(err.to_string().contains("post a version"), "{err}");
}
