//! A card that waits goes back to Running when the agent shows life again.

use toknila_core::agent::AgentEvent;
use toknila_core::hook::Sign;
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

fn sign(signal: &str) -> Sign<'_> {
    Sign::of(signal)
}

#[test]
fn an_mcp_event_leaves_a_card_waiting_on_a_question_or_a_permission() {
    for permission in [true, false] {
        let mut mem = running();
        ask(&mut mem, "may I run rm?", permission);
        let waiting = a(&mem);
        assert_eq!(waiting.status, TicketStatus::Waiting);
        note(&mut mem);
        let t = a(&mem);
        agent::ticket_event(&mut mem, &t, AgentEvent::Started).expect("started");
        let t = a(&mem);
        assert_eq!(
            (t.status, t.status_reason),
            (TicketStatus::Waiting, waiting.status_reason),
            "permission: {permission}"
        );
    }
}

#[test]
fn a_hook_of_work_never_answers_a_question() {
    let mut mem = running();
    ask(&mut mem, "which db?", false);
    let t = a(&mem);
    for signal in ["hook:post_tool_use", "hook:user_prompt_submit", "api"] {
        agent::activity(&mut mem, &t, &sign(signal)).expect("activity");
        assert_eq!(a(&mem).status, TicketStatus::Waiting, "{signal}");
    }
}

#[test]
fn a_permission_answered_at_the_terminal_resumes_on_the_hook_of_its_call_only() {
    let mut mem = running();
    ask(&mut mem, "may I run ls?", true);
    let t = a(&mem);
    let unrelated = Sign {
        signal: "hook:post_tool_use",
        tool: Some("Bash: pwd"),
        prompted: Some("Bash: ls"),
    };
    agent::activity(&mut mem, &t, &unrelated).expect("activity");
    assert_eq!(a(&mem).status, TicketStatus::Waiting);
    agent::activity(&mut mem, &t, &sign("hook:user_prompt_submit")).expect("activity");
    assert_eq!(a(&mem).status, TicketStatus::Waiting);
    let prompted = Sign {
        tool: Some("Bash: ls"),
        ..unrelated
    };
    agent::activity(&mut mem, &t, &prompted).expect("activity");
    let t = a(&mem);
    assert_eq!((t.status, t.status_reason), (TicketStatus::Running, None));
}

#[test]
fn a_hand_in_overtakes_an_open_question_and_goes_on() {
    let mut mem = running();
    ask(&mut mem, "which db?", false);
    let t = a(&mem);
    let summary = toknila_core::handin::from_plain("done");
    agent::ticket_event(&mut mem, &t, AgentEvent::Finished { summary }).expect("finished");
    assert_eq!(a(&mem).status, TicketStatus::Review);
    let overtaken: Vec<_> = mem
        .comments
        .iter()
        .filter(|c| c.author == toknila_core::Actor::System)
        .filter(|c| c.body.gist.contains("overtaken"))
        .collect();
    assert_eq!(overtaken.len(), 1, "{:?}", mem.comments);
    assert!(
        overtaken[0]
            .body
            .what
            .as_deref()
            .is_some_and(|w| w.contains("which db?"))
    );
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
        agent::activity(&mut mem, &t, &sign(quiet)).expect("activity");
        assert_eq!(a(&mem).status, TicketStatus::Waiting, "{quiet}");
    }
    agent::activity(&mut mem, &t, &sign("hook:post_tool_use")).expect("activity");
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
    agent::activity(&mut mem, &t, &sign("hook:user_prompt_submit")).expect("activity");
    assert_eq!(a(&mem).status, TicketStatus::Running);
}

#[test]
fn a_question_while_silent_replaces_the_silent_reason() {
    let mut mem = running();
    let t = a(&mem);
    let reason = WaitingReason::Silent;
    tickets::act(&mut mem, &t, TicketEvent::AskHuman { reason }).expect("silent");
    ask(&mut mem, "which db?", false);
    let want = WaitingReason::Question {
        text: "which db?".to_owned(),
    };
    assert_eq!(a(&mem).status_reason, Some(StatusReason::Waiting(want)));
}

#[test]
fn a_silent_give_up_never_clears_an_open_question() {
    let mut mem = running();
    ask(&mut mem, "which db?", false);
    let t = a(&mem);
    let reason = WaitingReason::Silent;
    tickets::act(&mut mem, &t, TicketEvent::AskHuman { reason }).expect("kept");
    let want = WaitingReason::Question {
        text: "which db?".to_owned(),
    };
    assert_eq!(a(&mem).status_reason, Some(StatusReason::Waiting(want)));
}

#[test]
fn activity_of_a_running_card_changes_nothing() {
    let mut mem = running();
    let t = a(&mem);
    agent::activity(&mut mem, &t, &sign("hook:post_tool_use")).expect("activity");
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
