//! A card that waits goes back to Running when the agent shows life again.

use toknila_core::agent::AgentEvent;
use toknila_core::machine::TicketEvent;
use toknila_core::{CommentBody, StatusReason, TicketStatus, WaitingReason};

use super::Mem;
use super::runs::{a, settle, with_agents};
use crate::recovery::recover;
use crate::{agent, tickets};

fn running() -> Mem {
    let mut mem = with_agents();
    recover(&mut mem).expect("settle");
    settle(&mut mem);
    mem
}

fn ask(mem: &mut Mem, text: &str, permission: bool) {
    let t = a(mem);
    let event = AgentEvent::AskHuman {
        text: text.to_owned(),
        permission,
        detail: None,
    };
    agent::ticket_event(mem, &t, event).expect("ask_human");
}

fn note(mem: &mut Mem) {
    let t = a(mem);
    let body = CommentBody {
        gist: "back at it".to_owned(),
        ..Default::default()
    };
    agent::ticket_event(mem, &t, AgentEvent::Note { body }).expect("note");
}

#[test]
fn an_mcp_event_after_ask_human_returns_the_card_to_running() {
    let mut mem = running();
    ask(&mut mem, "may I run rm?", true);
    assert_eq!(a(&mem).status, TicketStatus::Waiting);
    note(&mut mem);
    let t = a(&mem);
    assert_eq!((t.status, t.status_reason), (TicketStatus::Running, None));
}

#[test]
fn asking_again_while_waiting_keeps_it_waiting() {
    let mut mem = running();
    ask(&mut mem, "first", false);
    let t = a(&mem);
    let event = AgentEvent::AskHuman {
        text: "second".to_owned(),
        permission: false,
        detail: None,
    };
    // Still waiting; the second question is refused by the machine, not turned into a resume.
    assert!(agent::ticket_event(&mut mem, &t, event).is_err());
    assert_eq!(a(&mem).status, TicketStatus::Waiting);
}

#[test]
fn a_hook_that_means_work_resumes_and_one_that_means_waiting_does_not() {
    let mut mem = running();
    ask(&mut mem, "may I run rm?", true);
    let t = a(&mem);
    for quiet in ["hook:stop", "hook:notification", "start"] {
        agent::activity(&mut mem, &t, quiet).expect("activity");
        assert_eq!(a(&mem).status, TicketStatus::Waiting, "{quiet}");
    }
    agent::activity(&mut mem, &t, "hook:post_tool_use").expect("activity");
    assert_eq!(a(&mem).status, TicketStatus::Running);
}

#[test]
fn a_silent_card_returns_to_running_on_activity() {
    let mut mem = running();
    let t = a(&mem);
    let reason = WaitingReason::Silent;
    tickets::act(&mut mem, &t, TicketEvent::AskHuman { reason }).expect("silent");
    assert_eq!(
        a(&mem).status_reason,
        Some(StatusReason::Waiting(WaitingReason::Silent))
    );
    agent::activity(&mut mem, &t, "hook:user_prompt_submit").expect("activity");
    assert_eq!(a(&mem).status, TicketStatus::Running);
}

#[test]
fn activity_of_a_running_card_changes_nothing() {
    let mut mem = running();
    let t = a(&mem);
    agent::activity(&mut mem, &t, "hook:post_tool_use").expect("activity");
    assert_eq!(a(&mem).status, TicketStatus::Running);
}

#[test]
fn talking_with_the_owner_does_not_resume_a_waiting_card() {
    use toknila_core::ThreadKind;
    let mut mem = running();
    ask(&mut mem, "which flow?", false);
    let t = a(&mem);
    let event = AgentEvent::OpenThread {
        kind: ThreadKind::Design,
        title: "Flow".to_owned(),
    };
    agent::ticket_event(&mut mem, &t, event).expect("thread");
    assert_eq!(a(&mem).status, TicketStatus::Waiting);
}
