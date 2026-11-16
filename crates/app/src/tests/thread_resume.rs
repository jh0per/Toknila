//! A person's decision on a thread reaches the card's worker as an instruction comment: it
//! answers the worker's own question, and nothing else is resumed.

use toknila_core::agent::AgentEvent;
use toknila_core::{CommentKind, StatusReason, Thread, ThreadKind, TicketStatus, WaitingReason};

use super::Mem;
use super::runs::{a, settle, with_agents};
use crate::agent;
use crate::recovery::recover;
use crate::runner::RunnerState;
use crate::threads::{self, NewVersion};

const TEXT: &str = r#"Thread 1 "Flow" version 1 is approved with these decisions of the person. Where they differ from version 1, the decisions win; say in the hand-in how you followed each: Use the left layout. Continue."#;

/// A running card with an idle agent that takes typed input, and a design thread with a
/// version and a message from the person.
fn designed() -> (Mem, Thread) {
    let mut mem = with_agents();
    recover(&mut mem).expect("settle");
    settle(&mut mem);
    mem.runner.reach.set(RunnerState {
        hooks: true,
        idle: true,
        ..Default::default()
    });
    let t = a(&mem);
    let event = AgentEvent::OpenThread {
        kind: ThreadKind::Design,
        title: "Flow".to_owned(),
    };
    agent::ticket_event(&mut mem, &t, event).expect("thread");
    let th = mem.threads.last().expect("thread").clone();
    let version = NewVersion {
        format: toknila_core::ArtifactFormat::Mermaid,
        body: "graph TD; A-->B".to_owned(),
        message: None,
    };
    mem.actor = toknila_core::Actor::Agent;
    threads::post_version(&mut mem, &t, &th, version).expect("version");
    mem.actor = toknila_core::Actor::Human;
    mem.now = toknila_core::Timestamp(mem.now.0 + 1_000);
    threads::post_message(&mut mem, &t, &th, "Use the left layout.").expect("message");
    (mem, th)
}

fn ask(mem: &mut Mem, permission: bool) {
    let t = a(mem);
    let event = AgentEvent::AskHuman {
        text: "Design v1 is in thread 1. Do you approve it?".to_owned(),
        permission,
        detail: None,
        choices: Vec::new(),
    };
    agent::ticket_event(mem, &t, event).expect("ask_human");
    assert_eq!(a(mem).status, TicketStatus::Waiting);
}

fn instructions(mem: &Mem) -> Vec<String> {
    mem.comments
        .iter()
        .filter(|c| c.kind == CommentKind::Instruction)
        .map(|c| c.body.gist.clone())
        .collect()
}

#[test]
fn approving_a_thread_answers_the_question_a_waiting_worker_asked() {
    let (mut mem, th) = designed();
    ask(&mut mem, false);
    let t = a(&mem);
    threads::approve(&mut mem, &t, &th).expect("approve");
    settle(&mut mem);
    assert_eq!(instructions(&mem), [TEXT]);
    let now = a(&mem);
    assert_eq!(
        (now.status, now.status_reason),
        (TicketStatus::Running, None)
    );
    let typed = mem.runner.written.borrow()[0].clone();
    assert_eq!(
        String::from_utf8(typed).expect("utf8"),
        format!("# [instruction] {TEXT}")
    );
    assert!(
        mem.comments
            .iter()
            .filter(|c| c.kind == CommentKind::Instruction)
            .all(|c| c.delivered_at.is_some())
    );
}

#[test]
fn approving_a_thread_of_a_running_card_leaves_a_pending_instruction() {
    let (mut mem, th) = designed();
    mem.runner.reach.set(RunnerState {
        hooks: true,
        idle: false,
        ..Default::default()
    });
    let t = a(&mem);
    threads::approve(&mut mem, &t, &th).expect("approve");
    settle(&mut mem);
    assert_eq!(a(&mem).status, TicketStatus::Running);
    assert_eq!(instructions(&mem), [TEXT]);
    assert!(
        mem.comments
            .iter()
            .filter(|c| c.kind == CommentKind::Instruction)
            .all(|c| c.delivered_at.is_none())
    );
    let claimed = crate::instructions::claim(&mut mem, &t).expect("claim");
    assert_eq!(claimed.count, 1);
}

#[test]
fn approving_a_thread_of_a_card_in_review_or_backlog_records_nothing_extra() {
    for status in [TicketStatus::Review, TicketStatus::Backlog] {
        let (mut mem, th) = designed();
        let t = a(&mem);
        let i = mem
            .view
            .tickets
            .iter()
            .position(|x| x.id == t.id)
            .expect("a");
        mem.view.tickets[i].status = status;
        threads::approve(&mut mem, &t, &th).expect("approve");
        settle(&mut mem);
        assert!(instructions(&mem).is_empty(), "{status:?}");
        assert_eq!(a(&mem).status, status);
    }
}

#[test]
fn a_permission_prompt_is_not_answered_by_a_thread() {
    let (mut mem, th) = designed();
    ask(&mut mem, true);
    let t = a(&mem);
    threads::approve(&mut mem, &t, &th).expect("approve");
    settle(&mut mem);
    assert!(instructions(&mem).is_empty());
    let now = a(&mem);
    assert_eq!(now.status, TicketStatus::Waiting);
    assert!(matches!(
        now.status_reason,
        Some(StatusReason::Waiting(WaitingReason::Permission { .. }))
    ));
}

#[test]
fn approving_without_a_newer_person_message_keeps_the_plain_text() {
    let (mut mem, th) = designed();
    // The version is posted again after the person's message: the message is older now.
    let t = a(&mem);
    mem.actor = toknila_core::Actor::Agent;
    mem.now = toknila_core::Timestamp(mem.now.0 + 1_000);
    let version = NewVersion {
        format: toknila_core::ArtifactFormat::Mermaid,
        body: "graph TD; A-->C".to_owned(),
        message: None,
    };
    threads::post_version(&mut mem, &t, &th, version).expect("version 2");
    mem.actor = toknila_core::Actor::Human;
    ask(&mut mem, false);
    threads::approve(&mut mem, &t, &th).expect("approve");
    settle(&mut mem);
    assert_eq!(
        instructions(&mem),
        [r#"Thread 1 "Flow" version 2 is approved. Use the left layout. Continue."#]
    );
}

/// The brief threads of the reviewer that started after the hand-in of card A.
fn reviewer_threads(mem: &mut Mem) -> Vec<(u32, Option<String>)> {
    mem.project.check_command = Some("cargo xtask verify".to_owned());
    mem.project.default_review_mode = toknila_core::ReviewMode::Both;
    super::runs::finish(mem, "done".to_owned());
    settle(mem);
    let started = mem.runner.started.borrow();
    let req = started
        .iter()
        .find(|r| r.role == toknila_core::AgentRole::Reviewer)
        .expect("a reviewer started");
    req.brief
        .threads
        .iter()
        .map(|t| (t.number, t.approval_message.clone()))
        .collect()
}

#[test]
fn the_reviewers_thread_record_carries_the_approval_message_only_when_it_is_newer() {
    let (mut mem, th) = designed();
    let t = a(&mem);
    threads::approve(&mut mem, &t, &th).expect("approve");
    assert_eq!(
        reviewer_threads(&mut mem),
        [(1, Some("Use the left layout.".to_owned()))]
    );

    // Control: a version newer than the message leaves no decisions to show.
    let (mut mem, th) = designed();
    let t = a(&mem);
    mem.actor = toknila_core::Actor::Agent;
    mem.now = toknila_core::Timestamp(mem.now.0 + 1_000);
    let version = NewVersion {
        format: toknila_core::ArtifactFormat::Mermaid,
        body: "graph TD; A-->C".to_owned(),
        message: None,
    };
    threads::post_version(&mut mem, &t, &th, version).expect("version 2");
    mem.actor = toknila_core::Actor::Human;
    threads::approve(&mut mem, &t, &th).expect("approve");
    assert_eq!(reviewer_threads(&mut mem), [(1, None)]);
}

#[test]
fn asking_for_changes_reaches_the_worker_too() {
    let (mut mem, th) = designed();
    ask(&mut mem, false);
    let t = a(&mem);
    threads::request_changes(&mut mem, &t, &th, "Make it wider").expect("changes");
    settle(&mut mem);
    assert_eq!(
        instructions(&mem),
        [r#"Thread 1 "Flow": changes requested. Make it wider. Continue."#]
    );
    assert_eq!(a(&mem).status, TicketStatus::Running);
}
