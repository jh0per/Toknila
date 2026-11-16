//! A resume is a start: an agent brought back from a pause runs only within the project's limit.
//! With one slot, card A paused on a question and card B running, the answer queues A, and B's end
//! starts it with the answer in its kickoff.

use toknila_core::agent::AgentEvent;
use toknila_core::machine::TicketEvent;
use toknila_core::{CommentBody, CommentKind, TicketStatus};

use super::Mem;
use super::runs::{a, act, settle, with_agents};
use crate::recovery::recover;
use crate::{agent, paused, schedule, tickets};

fn b(mem: &Mem) -> toknila_core::Ticket {
    mem.ticket_by_title("B")
}

/// One slot; A asked a question and its agent was paused; B took the slot A freed.
fn a_paused_b_running() -> Mem {
    let mut mem = with_agents();
    mem.project.max_parallel = 1;
    mem.view.edges.clear();
    recover(&mut mem).expect("start");
    settle(&mut mem);
    let t = a(&mem);
    assert_eq!(t.status, TicketStatus::Running);
    let ask = AgentEvent::AskHuman {
        text: "which db?".to_owned(),
        permission: false,
        detail: None,
        choices: Vec::new(),
    };
    agent::ticket_event(&mut mem, &t, ask).expect("ask");
    let t = a(&mem);
    // The host ends the process of a waiting card, then tells the app.
    mem.runner.live.borrow_mut().clear();
    paused::pause(&mut mem, t.id, Some("sess".to_owned())).expect("pause");
    settle(&mut mem);
    assert!(a(&mem).agent_paused.is_some());
    assert_eq!(b(&mem).status, TicketStatus::Running, "B took the slot");
    mem.runner.started.borrow_mut().clear();
    mem
}

fn answer(mem: &mut Mem) {
    let t = a(mem);
    let body = CommentBody {
        gist: "use sqlite".to_owned(),
        ..CommentBody::default()
    };
    tickets::comment(mem, &t, CommentKind::Instruction, body).expect("answer");
}

fn cancel_b(mem: &mut Mem) {
    let t = b(mem);
    let event = TicketEvent::Cancel {
        reason: "done".to_owned(),
    };
    tickets::act(mem, &t, event).expect("cancel");
    settle(mem);
}

#[test]
fn an_answer_queues_a_paused_agent_while_every_slot_is_busy_and_the_end_of_the_other_starts_it() {
    let mut mem = a_paused_b_running();
    answer(&mut mem);
    settle(&mut mem);
    assert!(
        mem.runner.started.borrow().is_empty(),
        "no runner past the limit: {:?}",
        mem.runner.started.borrow()
    );
    let id = a(&mem).id;
    let line = schedule::queue_positions(&mem).expect("positions");
    assert_eq!(line.get(&id), Some(&1));
    assert_eq!(a(&mem).status, TicketStatus::Waiting);

    cancel_b(&mut mem);
    let started = mem.runner.started.borrow().clone();
    let [req] = started.as_slice() else {
        panic!("A starts once: {started:?}");
    };
    assert_eq!(req.ticket, id);
    let resume = req.brief.resume.clone().expect("a resume");
    assert!(resume.input.contains("use sqlite"), "{}", resume.input);
    assert_eq!(a(&mem).status, TicketStatus::Running);
}

#[test]
fn a_permission_answer_queues_the_same_way() {
    let mut mem = a_paused_b_running();
    let t = a(&mem);
    paused::answer_permission(
        &mut mem,
        &t,
        toknila_core::question::AnswerKind::Allow,
        None,
    )
    .expect("answer");
    settle(&mut mem);
    assert!(mem.runner.started.borrow().is_empty());
    cancel_b(&mut mem);
    assert_eq!(mem.runner.started.borrow().len(), 1);
}

#[test]
fn a_late_signal_of_the_ended_process_does_not_bring_a_paused_agent_back() {
    use toknila_core::hook::Sign;
    let mut mem = a_paused_b_running();
    let t = a(&mem);
    agent::activity(&mut mem, &t, &Sign::of("hook:post_tool_use")).expect("activity");
    settle(&mut mem);
    let t = a(&mem);
    assert_eq!(t.status, TicketStatus::Waiting);
    assert!(t.agent_paused.is_some(), "still paused");
    assert!(mem.runner.started.borrow().is_empty());
    let line = schedule::queue_positions(&mem).expect("positions");
    assert!(line.get(&t.id).is_none(), "nobody wants it yet");
}

#[test]
fn a_lost_runners_replacement_waits_for_a_slot_too() {
    let mut mem = a_paused_b_running();
    // The daemon restarts: A's agent is paused and lost; B was running and is lost too, and both
    // want their agents back with one slot between them.
    mem.runner.live.borrow_mut().clear();
    mem.deferred.clear();
    let report = recover(&mut mem).expect("restart");
    assert!(!report.lost.is_empty());
    settle(&mut mem);
    let live = mem.runner.live.borrow().len();
    assert!(live <= 1, "{live} runners at a limit of 1");
    let _ = act;
}
