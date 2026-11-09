//! What a run that starts after a Return, or with instructions nobody has acted on, is told: the
//! kickoff and the instructions file carry them, and they are delivered once.

use toknila_core::AgentRole;
use toknila_core::agent::AgentEvent;
use toknila_core::machine::TicketEvent;

use super::Mem;
use super::runs::{a, act, settle, with_agents};
use crate::recovery::recover;
use crate::{agent, instructions, tickets};

fn in_review() -> Mem {
    let mut mem = with_agents();
    mem.project.default_review_mode = toknila_core::ReviewMode::Human;
    recover(&mut mem).expect("settle");
    settle(&mut mem);
    let t = a(&mem);
    // The hand-in at a known commit, as the daemon records it.
    agent::ticket_event_at(
        &mut mem,
        &t,
        AgentEvent::Finished {
            summary: "done".to_owned(),
        },
        Some("f8afe06c0ffee".to_owned()),
    )
    .expect("finished");
    settle(&mut mem);
    mem
}

fn last_worker_brief(mem: &Mem) -> crate::runner::RunBrief {
    mem.runner
        .started
        .borrow()
        .iter()
        .rfind(|r| r.role == AgentRole::Worker)
        .expect("a worker started")
        .brief
        .clone()
}

#[test]
fn the_hand_in_commit_is_remembered_and_a_return_restarts_the_worker_with_it() {
    let mut mem = in_review();
    assert_eq!(a(&mem).review.handed_in.as_deref(), Some("f8afe06c0ffee"));
    let comment = "Rename the flag".to_owned();
    act(&mut mem, TicketEvent::Return { comment });
    settle(&mut mem);
    let brief = last_worker_brief(&mem);
    let returned = brief.returned.expect("the kickoff names the Return");
    // Round 1 is the hand-in's own check (a card with a ship is brought up to its base and
    // checked); the Return is round 2.
    assert_eq!(returned.round, 2);
    assert_eq!(returned.commit.as_deref(), Some("f8afe06c0ffee"));
    assert_eq!(returned.text, "Rename the flag");
    assert_eq!(a(&mem).review.handed_in.as_deref(), Some("f8afe06c0ffee"));
}

#[test]
fn the_pending_comments_go_into_the_new_run_and_are_delivered_once() {
    let mut mem = in_review();
    mem.runner.reach.set(crate::runner::RunnerState {
        hooks: true,
        idle: true,
        ..Default::default()
    });
    let comment = "Rename the flag\nand mind the docs".to_owned();
    act(&mut mem, TicketEvent::Return { comment });
    settle(&mut mem);
    let brief = last_worker_brief(&mem);
    assert_eq!(brief.waiting_ids.len(), 1, "the Return's comment is taken");
    assert!(
        mem.runner.written.borrow().is_empty(),
        "the new run read it; nothing is typed again"
    );
    let delivered = mem
        .kinds()
        .iter()
        .filter(|k| *k == "comment.delivered")
        .count();
    assert_eq!(delivered, 1);
    let t = a(&mem);
    let claimed = instructions::claim(&mut mem, &t).expect("claim");
    assert_eq!(claimed.count, 0, "the Stop hook has nothing left to type");
}

#[test]
fn an_agent_that_is_not_told_its_brief_still_gets_the_return_typed() {
    let mut mem = in_review();
    mem.runner.unbriefed.set(true);
    mem.runner.reach.set(crate::runner::RunnerState {
        hooks: false,
        idle: true,
        ..Default::default()
    });
    let comment = "Rename the flag".to_owned();
    act(&mut mem, TicketEvent::Return { comment });
    settle(&mut mem);
    let written = mem.runner.written.borrow().clone();
    assert_eq!(written[0], b"# [instruction] Rename the flag");
    let t = a(&mem);
    assert_eq!(instructions::claim(&mut mem, &t).expect("claim").count, 0);
}

#[test]
fn context_given_while_nobody_works_is_in_the_next_run_with_the_return() {
    let mut mem = in_review();
    let t = a(&mem);
    let body = toknila_core::CommentBody {
        gist: "Mind the Windows paths".to_owned(),
        ..Default::default()
    };
    tickets::comment(&mut mem, &t, toknila_core::CommentKind::Instruction, body).expect("comment");
    settle(&mut mem);
    let comment = "Rename the flag".to_owned();
    act(&mut mem, TicketEvent::Return { comment });
    settle(&mut mem);
    let brief = last_worker_brief(&mem);
    assert_eq!(brief.waiting_ids.len(), 2, "context and Return");
    assert!(
        brief.waiting.iter().any(|w| w.contains("Windows paths")),
        "{:?}",
        brief.waiting
    );
    let t = a(&mem);
    let claimed = instructions::claim(&mut mem, &t).expect("claim");
    assert_eq!(claimed.count, 0);
}
