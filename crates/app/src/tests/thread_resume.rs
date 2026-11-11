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

const TEXT: &str = r#"Thread 1 "Flow" version 1 is approved. Use the left layout. Continue."#;

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
    threads::post_message(&mut mem, &t, &th, "Use the left layout.").expect("message");
    (mem, th)
}

fn ask(mem: &mut Mem, permission: bool) {
    let t = a(mem);
    let event = AgentEvent::AskHuman {
        text: "Design v1 is in thread 1. Do you approve it?".to_owned(),
        permission,
        detail: None,
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
