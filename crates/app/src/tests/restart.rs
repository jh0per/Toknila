//! An update restarts the daemon, and the restart does not stop a story: a lost agent comes back
//! by itself, a loss twice in a row is a loop, and an unblock never leaves a card without an agent.

use toknila_core::agent::AgentEvent;
use toknila_core::instructions::RESTARTED;
use toknila_core::machine::TicketEvent;
use toknila_core::{
    BlockedReason, CommentBody, CommentKind, StatusReason, Ticket, TicketStatus, Timestamp,
    WaitingReason,
};

use super::Mem;
use super::runs::{a, act, settle, with_agents};
use crate::recovery::recover;
use crate::{agent, tickets};

const MINUTE: i64 = 60_000;

/// DEMO-1 runs under a process; then the daemon dies with the process and starts again.
fn started() -> Mem {
    let mut mem = with_agents();
    mem.mem_start();
    mem
}

impl Mem {
    fn mem_start(&mut self) {
        recover(self).expect("first start");
        settle(self);
        assert_eq!(a(self).status, TicketStatus::Running);
    }

    /// The daemon died: no process is left and nothing was told. The next start recovers.
    fn restart(&mut self) -> crate::recovery::Recovery {
        self.runner.live.borrow_mut().clear();
        self.runner.started.borrow_mut().clear();
        self.deferred.clear();
        recover(self).expect("restart")
    }

    fn resume_requests(&self) -> Vec<crate::runner::RunRequest> {
        self.runner
            .started
            .borrow()
            .iter()
            .filter(|r| r.brief.resume.is_some())
            .cloned()
            .collect()
    }
}

/// A card whose agent came back after a loss, and when.
fn resumed_after_a_loss() -> (Mem, Timestamp) {
    let mut mem = started();
    mem.now = Timestamp(1_000_000);
    mem.restart();
    settle(&mut mem);
    let first = a(&mem).lost_resume.expect("resumed after a loss");
    (mem, first)
}

fn reason(t: &Ticket) -> BlockedReason {
    let Some(StatusReason::Blocked { reason, .. }) = t.status_reason.clone() else {
        panic!("{} is not blocked: {:?}", t.key, t.status_reason);
    };
    reason
}

#[test]
fn a_restart_pauses_a_running_card_and_a_replacement_run_starts_with_the_restart_sentence() {
    let mut mem = started();
    let report = mem.restart();
    assert_eq!(report.lost, vec!["DEMO-1".to_owned()]);
    // Paused and wanting its agent back: the schedule had a free slot, so it runs already.
    let t = a(&mem);
    assert_eq!(t.status, TicketStatus::Running);
    assert_eq!(t.agent_paused, None, "the replacement run was queued");
    assert!(
        t.lost_resume.is_some(),
        "the resume that followed a loss is kept on the card"
    );
    settle(&mut mem);
    let runs = mem.resume_requests();
    assert_eq!(runs.len(), 1, "started once: {runs:?}");
    let resume = runs[0].brief.resume.clone().expect("resume");
    assert!(resume.lost);
    assert!(resume.input.contains(RESTARTED), "{}", resume.input);
    assert_eq!(a(&mem).status, TicketStatus::Running);
}

#[test]
fn a_restart_leaves_a_waiting_card_with_its_question_and_the_answer_starts_its_agent() {
    let mut mem = started();
    let t = a(&mem);
    let ask = AgentEvent::AskHuman {
        text: "which db?".to_owned(),
        permission: false,
        detail: None,
        choices: Vec::new(),
    };
    agent::ticket_event(&mut mem, &t, ask).expect("ask");
    let waiting = a(&mem);
    assert_eq!(waiting.status, TicketStatus::Waiting);
    let report = mem.restart();
    assert_eq!(report.lost, vec!["DEMO-1".to_owned()]);
    let t = a(&mem);
    assert_eq!(t.status, TicketStatus::Waiting);
    assert_eq!(t.status_reason, waiting.status_reason);
    assert!(t.agent_paused.as_ref().is_some_and(|p| !p.resume && p.lost));
    settle(&mut mem);
    assert!(
        mem.resume_requests().is_empty(),
        "nobody starts it before the answer"
    );
    // The answer starts it and reaches it.
    let body = CommentBody {
        gist: "use sqlite".to_owned(),
        ..CommentBody::default()
    };
    tickets::comment(&mut mem, &t, CommentKind::Instruction, body).expect("answer");
    settle(&mut mem);
    let runs = mem.resume_requests();
    assert_eq!(runs.len(), 1, "{runs:?}");
    let resume = runs[0].brief.resume.clone().expect("resume");
    assert!(resume.input.contains("use sqlite"), "{}", resume.input);
    assert_eq!(a(&mem).status, TicketStatus::Running);
    assert!(
        mem.comments
            .iter()
            .filter(|c| c.kind == CommentKind::Instruction)
            .all(|c| c.delivered_at.is_some()),
        "{:?}",
        mem.comments
    );
}

#[test]
fn a_loss_within_ten_minutes_of_a_resume_after_a_loss_blocks_the_card_with_both_times() {
    // Control: ten minutes after the resume, the same loss is a single loss and the card comes back.
    let (mut later, first) = resumed_after_a_loss();
    later.now = Timestamp(first.0 + 10 * MINUTE);
    later.restart();
    settle(&mut later);
    assert_eq!(a(&later).status, TicketStatus::Running);
    assert_eq!(later.resume_requests().len(), 1);
    // Lost again five minutes after the resume.
    let (mut mem, first) = resumed_after_a_loss();
    mem.now = Timestamp(first.0 + 5 * MINUTE);
    mem.restart();
    let t = a(&mem);
    assert_eq!(t.status, TicketStatus::Blocked);
    let BlockedReason::RunnerLost {
        first: f,
        second: s,
    } = reason(&t)
    else {
        panic!("runner lost: {:?}", t.status_reason);
    };
    assert_eq!((f, s), (Some(first), Some(Timestamp(first.0 + 5 * MINUTE))));
    let text = reason(&t).to_string();
    assert!(
        text.contains(&first.utc_text()) && text.contains("unblock starts it again"),
        "{text}"
    );
    settle(&mut mem);
    assert!(mem.resume_requests().is_empty(), "no third start by itself");
}

#[test]
fn unblocking_a_lost_runner_pauses_the_agent_and_never_leaves_a_bare_running_card() {
    let mut mem = started();
    mem.restart();
    settle(&mut mem);
    mem.runner.live.borrow_mut().clear();
    mem.restart();
    assert_eq!(a(&mem).status, TicketStatus::Blocked);
    // Unblock: a Running card is paused with the resume wanted; the schedule starts the agent.
    mem.deferred.clear();
    act(&mut mem, TicketEvent::Unblock);
    let t = a(&mem);
    assert_eq!(t.status, TicketStatus::Running);
    assert!(
        t.agent_paused.is_some() || !mem.deferred.is_empty(),
        "Running with neither a paused agent nor a start queued: {t:?}"
    );
    settle(&mut mem);
    assert_eq!(mem.resume_requests().len(), 1);
    assert!(mem.runner.live.borrow().iter().any(|(id, _)| *id == t.id));
}

#[test]
fn unblocking_a_lost_waiting_card_keeps_the_question_and_pauses_the_agent() {
    let mut mem = started();
    let t = a(&mem);
    let ask = AgentEvent::AskHuman {
        text: "which db?".to_owned(),
        permission: false,
        detail: None,
        choices: Vec::new(),
    };
    agent::ticket_event(&mut mem, &t, ask).expect("ask");
    let reason = BlockedReason::RunnerLost {
        first: Some(Timestamp(1)),
        second: Some(Timestamp(2)),
    };
    act(&mut mem, TicketEvent::Block { reason });
    act(&mut mem, TicketEvent::Unblock);
    let t = a(&mem);
    assert_eq!(t.status, TicketStatus::Waiting);
    assert!(matches!(
        t.status_reason,
        Some(StatusReason::Waiting(WaitingReason::Question { .. }))
    ));
    assert!(t.agent_paused.is_some());
}

#[test]
fn a_simulated_project_loses_nothing_in_a_restart() {
    let mut mem = with_agents();
    mem.project.default_profile = "sim".to_owned();
    recover(&mut mem).expect("start");
    let report = recover(&mut mem).expect("restart");
    assert!(report.lost.is_empty());
    let t = a(&mem);
    assert_eq!((t.status, t.agent_paused), (TicketStatus::Running, None));
}
