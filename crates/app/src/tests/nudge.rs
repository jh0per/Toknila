//! The silence loop with a fake clock (`Mem::now`) and a fake runner.

use toknila_core::nudge::Decision;
use toknila_core::{
    Actor, CommentBody, CommentKind, Nudge, StatusReason, TicketStatus, Timestamp, WaitingReason,
};

use super::Mem;
use super::runs::{a, settle, with_agents};
use crate::nudge::{self, Reported};
use crate::recovery::recover;
use crate::runner::{Runner, RunnerState};
use crate::tickets;

const START: i64 = 1_000;

fn cfg() -> Nudge {
    Nudge::default()
}

/// A running agent, started at `START`, whose terminal has gone quiet since then.
fn quiet_run() -> Mem {
    let mut mem = with_agents();
    recover(&mut mem).expect("settle");
    settle(&mut mem);
    mem.runner.reach.set(RunnerState {
        hooks: true,
        idle: true,
        ..Default::default()
    });
    mem
}

fn reported() -> Reported {
    Reported {
        started: Timestamp(START),
        mcp: None,
        hook: Some(Timestamp(START)),
        screen_open: false,
    }
}

/// Moves the fake clock to `secs` after the start, then checks.
fn check_at(mem: &mut Mem, secs: i64) -> Decision {
    check_with(mem, secs, reported())
}

/// [`check_at`] with the host's facts given.
fn check_with(mem: &mut Mem, secs: i64, reported: Reported) -> Decision {
    mem.now = Timestamp(START + secs * 1000);
    mem.runner.reach.set(RunnerState {
        quiet_ms: u64::try_from(secs * 1000).expect("positive"),
        ..mem.runner.reach.get()
    });
    let t = a(mem);
    nudge::check(mem, mem.runner.state(t.id), t.id, &cfg(), reported, mem.now).expect("check")
}

#[test]
fn a_run_without_a_sign_of_life_or_with_an_open_screen_question_is_never_nudged() {
    let mut mem = quiet_run();
    let no_life = Reported {
        hook: None,
        ..reported()
    };
    assert_eq!(check_with(&mut mem, 5000, no_life), Decision::Wait);
    let open = Reported {
        screen_open: true,
        ..reported()
    };
    assert_eq!(check_with(&mut mem, 5000, open), Decision::Wait);
    // Control: the same quiet run, alive and with no question, is nudged.
    assert_eq!(check_with(&mut mem, 5000, reported()), Decision::Nudge);
}

fn apply(mem: &mut Mem, decision: Decision) {
    let t = a(mem);
    nudge::apply(mem, t.id, &cfg(), decision).expect("apply");
    settle(mem);
}

fn nudges(mem: &Mem) -> Vec<String> {
    mem.comments
        .iter()
        .filter(|c| c.author == Actor::System && c.kind == CommentKind::Instruction)
        .map(|c| c.body.gist.clone())
        .collect()
}

#[test]
fn a_quiet_agent_is_nudged_through_the_instruction_path() {
    let mut mem = quiet_run();
    assert_eq!(check_at(&mut mem, 119), Decision::Wait);
    let d = check_at(&mut mem, 120);
    assert_eq!(d, Decision::Nudge);
    apply(&mut mem, d);
    assert_eq!(nudges(&mem).len(), 1);
    let typed = String::from_utf8(mem.runner.written.borrow()[0].clone()).expect("text");
    assert!(typed.contains("Toknila tools"), "{typed}");
    assert!(mem.kinds().iter().any(|k| k == "comment.delivered"));
    // The nudge restarts the timer: nothing more for another threshold.
    assert_eq!(check_at(&mut mem, 239), Decision::Wait);
    assert_eq!(check_at(&mut mem, 240), Decision::Nudge);
}

#[test]
fn after_the_limit_the_card_waits_as_silent() {
    let mut mem = quiet_run();
    for at in [120, 240, 360] {
        let d = check_at(&mut mem, at);
        assert_eq!(d, Decision::Nudge, "at {at}");
        apply(&mut mem, d);
    }
    let d = check_at(&mut mem, 480);
    assert_eq!(d, Decision::GiveUp);
    apply(&mut mem, d);
    let t = a(&mem);
    assert_eq!(t.status, TicketStatus::Waiting);
    assert_eq!(
        t.status_reason,
        Some(StatusReason::Waiting(WaitingReason::Silent))
    );
    assert_eq!(nudges(&mem).len(), 3, "no fourth nudge");
}

#[test]
fn a_nudge_handed_over_after_a_question_does_not_clear_it() {
    use toknila_core::agent::AgentEvent;
    let mut mem = quiet_run();
    // The agent is mid-turn, so the nudge waits for its Stop hook.
    mem.runner.reach.set(RunnerState {
        hooks: true,
        idle: false,
        ..Default::default()
    });
    apply(&mut mem, Decision::Nudge);
    let t = a(&mem);
    let event = AgentEvent::AskHuman {
        text: "which db?".to_owned(),
        permission: false,
        detail: None,
        choices: Vec::new(),
    };
    crate::agent::ticket_event(&mut mem, &t, event).expect("ask_human");
    let t = a(&mem);
    let claimed = crate::instructions::claim(&mut mem, &t).expect("claim");
    assert_eq!(claimed.count, 1, "the nudge is handed over");
    let want = WaitingReason::Question {
        text: "which db?".to_owned(),
        choices: Vec::new(),
    };
    let t = a(&mem);
    assert_eq!(
        (t.status, t.status_reason),
        (TicketStatus::Waiting, Some(StatusReason::Waiting(want)))
    );
}

#[test]
fn a_live_child_process_suppresses_the_nudge() {
    let mut mem = quiet_run();
    mem.runner.reach.set(RunnerState {
        busy: true,
        ..mem.runner.reach.get()
    });
    assert_eq!(check_at(&mut mem, 5_000), Decision::Wait);
}

#[test]
fn a_delivered_instruction_counts_as_a_sign_of_life() {
    let mut mem = quiet_run();
    mem.now = Timestamp(START + 100_000);
    let t = a(&mem);
    let body = CommentBody {
        gist: "use rustls".to_owned(),
        ..Default::default()
    };
    tickets::comment(&mut mem, &t, CommentKind::Instruction, body).expect("comment");
    settle(&mut mem);
    // Delivered at 100 s: the threshold counts from there, not from the start.
    assert_eq!(check_at(&mut mem, 219), Decision::Wait);
    assert_eq!(check_at(&mut mem, 220), Decision::Nudge);
}

#[test]
fn only_a_running_ticket_is_watched() {
    let mut mem = quiet_run();
    let t = a(&mem);
    let reason = WaitingReason::Silent;
    tickets::act(
        &mut mem,
        &t,
        toknila_core::machine::TicketEvent::AskHuman { reason },
    )
    .expect("waits");
    assert_eq!(check_at(&mut mem, 5_000), Decision::Wait);
}
