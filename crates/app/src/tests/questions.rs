//! A person's answer to a run's question: journaled with the role, and a waiting worker is
//! Running again at once.

use toknila_core::agent::AgentEvent;
use toknila_core::question::{AnswerKind, QuestionKind};
use toknila_core::{Actor, AgentRole, StatusReason, TicketStatus, WaitingReason};

use super::Mem;
use super::runs::{a, settle, with_agents};
use crate::recovery::recover;
use crate::{ErrorKind, agent, questions};

fn running() -> Mem {
    let mut mem = with_agents();
    recover(&mut mem).expect("settle");
    settle(&mut mem);
    mem
}

fn ask(mem: &mut Mem) {
    let t = a(mem);
    let event = AgentEvent::AskHuman {
        text: "may I run rm?".to_owned(),
        permission: true,
        detail: None,
    };
    agent::ticket_event(mem, &t, event).expect("ask_human");
    assert_eq!(
        a(mem).status_reason,
        Some(StatusReason::Waiting(WaitingReason::Permission {
            text: "may I run rm?".to_owned(),
            detail: None
        }))
    );
}

#[test]
fn an_answer_to_a_waiting_worker_makes_it_running_at_once_and_is_journaled() {
    let mut mem = running();
    ask(&mut mem);
    mem.events.clear();
    let t = a(&mem);
    questions::answered(&mut mem, &t, AgentRole::Worker, AnswerKind::Allow).expect("answered");
    let now = a(&mem);
    assert_eq!(
        (now.status, now.status_reason),
        (TicketStatus::Running, None)
    );
    let kinds = mem.kinds();
    assert!(
        kinds.contains(&"ticket.question_answered".to_owned()),
        "{kinds:?}"
    );
    assert!(
        kinds.contains(&"ticket.status_changed".to_owned()),
        "{kinds:?}"
    );
}

#[test]
fn an_answer_to_a_reviewer_journals_the_role_and_leaves_the_card_alone() {
    let mut mem = running();
    let t = a(&mem);
    mem.events.clear();
    questions::answered(&mut mem, &t, AgentRole::Reviewer, AnswerKind::Deny).expect("answered");
    assert_eq!(a(&mem).status, TicketStatus::Running);
    assert_eq!(mem.kinds(), ["ticket.question_answered"]);
}

#[test]
fn an_answer_to_a_running_worker_changes_no_status() {
    let mut mem = running();
    let t = a(&mem);
    questions::answered(&mut mem, &t, AgentRole::Worker, AnswerKind::Enter).expect("answered");
    assert_eq!(a(&mem).status, TicketStatus::Running);
}

#[test]
fn a_workers_screen_question_moves_the_card_to_waiting_and_its_first_sign_of_life_back() {
    let mut mem = running();
    let t = a(&mem);
    mem.events.clear();
    let lines = vec!["Is this a project you trust?".to_owned()];
    questions::screen_raised(&mut mem, &t, AgentRole::Worker, &lines).expect("raised");
    let waiting = a(&mem);
    assert_eq!(waiting.status, TicketStatus::Waiting);
    assert_eq!(
        waiting.status_reason,
        Some(StatusReason::Waiting(WaitingReason::Screen))
    );
    assert_eq!(waiting.waiting_for(), "you: Claude Code asks on its screen");
    let kinds = mem.kinds();
    assert!(
        kinds.contains(&"ticket.question_raised".to_owned()),
        "{kinds:?}"
    );
    questions::screen_cleared(&mut mem, &t, AgentRole::Worker).expect("cleared");
    let now = a(&mem);
    assert_eq!(
        (now.status, now.status_reason),
        (TicketStatus::Running, None)
    );
}

#[test]
fn a_reviewers_screen_question_leaves_the_card_alone() {
    let mut mem = running();
    let t = a(&mem);
    questions::screen_raised(&mut mem, &t, AgentRole::Reviewer, &[]).expect("raised");
    assert_eq!(a(&mem).status, TicketStatus::Running);
}

#[test]
fn an_answer_to_a_screen_question_puts_the_card_back_to_running() {
    let mut mem = running();
    let t = a(&mem);
    questions::screen_raised(&mut mem, &t, AgentRole::Worker, &[]).expect("raised");
    questions::answered(&mut mem, &t, AgentRole::Worker, AnswerKind::Yes).expect("answered");
    let now = a(&mem);
    assert_eq!(
        (now.status, now.status_reason),
        (TicketStatus::Running, None)
    );
}

#[test]
fn only_a_person_answers() {
    let mut mem = running();
    ask(&mut mem);
    let t = a(&mem);
    mem.actor = Actor::Agent;
    let refused =
        questions::answered(&mut mem, &t, AgentRole::Worker, AnswerKind::Allow).expect_err("agent");
    assert_eq!(refused.kind, ErrorKind::Conflict);
    assert_eq!(a(&mem).status, TicketStatus::Waiting);
}

#[test]
fn raising_and_clearing_are_journaled_for_the_role() {
    let mut mem = running();
    let t = a(&mem);
    mem.events.clear();
    questions::raised(
        &mut mem,
        &t,
        AgentRole::Reviewer,
        QuestionKind::Screen,
        None,
    )
    .expect("raised");
    questions::cleared(&mut mem, &t, AgentRole::Reviewer).expect("cleared");
    assert_eq!(
        mem.kinds(),
        ["ticket.question_raised", "ticket.question_cleared"]
    );
}

#[test]
fn a_permission_request_leaves_no_comment_on_the_card_but_a_question_does() {
    let mut mem = running();
    ask(&mut mem);
    assert!(
        mem.comments.is_empty(),
        "a permission request is not a card comment: {:?}",
        mem.comments
    );
    let t = a(&mem);
    questions::answered(&mut mem, &t, AgentRole::Worker, AnswerKind::Allow).expect("answered");
    let t = a(&mem);
    let event = AgentEvent::AskHuman {
        text: "which database?".to_owned(),
        permission: false,
        detail: None,
    };
    agent::ticket_event(&mut mem, &t, event).expect("ask_human");
    assert_eq!(mem.comments.len(), 1, "an ask_human question is kept");
}
