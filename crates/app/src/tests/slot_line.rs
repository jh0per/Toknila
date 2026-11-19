//! Agents at once is a limit that holds on every path that starts or resumes a story's agent.
//! With one slot and agents that are processes (the fake runner), B works while A has to go on:
//! A's agent does not start, A says why it waits, and the end of B's agent starts it, first in
//! line. Then each event that frees a slot starts the next story in line, and a story's whole
//! life completes at a limit of 1 without waiting for itself.

use std::collections::HashSet;

use toknila_core::agent::AgentEvent;
use toknila_core::machine::TicketEvent;
use toknila_core::{
    AgentRole, BlockedReason, ProcessExit, ReviewMode, ReviewNote, ReviewTurn, StatusReason,
    Ticket, TicketStatus, Verdict,
};

use super::Mem;
use super::runs::{a, run_of, settle, with_agents};
use crate::chain::tell_ticket;
use crate::projects::{self, PatchProject};
use crate::recovery::recover;
use crate::runner::{RunBrief, RunEnded, RunRequest};
use crate::{agent, paused, runs, schedule, tickets};

fn b(mem: &Mem) -> Ticket {
    mem.ticket_by_title("B")
}

fn ticket_mut<'m>(mem: &'m mut Mem, title: &str) -> &'m mut Ticket {
    mem.view
        .tickets
        .iter_mut()
        .find(|t| t.title == title)
        .expect("card")
}

/// How many runs of `role` were started for `t`.
fn starts(mem: &Mem, t: &Ticket, role: AgentRole) -> usize {
    let started = mem.runner.started.borrow();
    started
        .iter()
        .filter(|r| r.ticket == t.id && r.role == role)
        .count()
}

/// Stories that hold a slot now (core's count).
fn used(mem: &Mem) -> u32 {
    schedule::plan_of(mem, &mem.project).expect("plan").used
}

fn start(mem: &mut Mem, t: &Ticket) {
    tickets::act(mem, t, TicketEvent::Request).expect("start");
    settle(mem);
}

/// The host reports that the process of `t`'s agent of `role` ended: one the host stopped
/// (`stopped`), or one that ended by itself.
fn exits(mem: &mut Mem, t: &Ticket, role: AgentRole, stopped: bool) {
    mem.runner.live.borrow_mut().remove(&(t.id, role));
    let run = run_of(&RunRequest {
        ticket: t.id,
        key: t.key.clone(),
        role,
        profile: "fake".to_owned(),
        dir: "/ships/x".to_owned(),
        brief: RunBrief::default(),
    });
    let exit = ProcessExit {
        code: Some(if stopped { 0 } else { 1 }),
        signal: None,
    };
    let ended = RunEnded {
        run,
        exit,
        stopped,
        refused: None,
    };
    runs::ended(mem, ended).expect("ended");
    settle(mem);
}

/// B hands in; its worker is stopped, and its end frees the slot.
fn b_hands_in(mem: &mut Mem) {
    let t = b(mem);
    let summary = toknila_core::handin::from_plain("done");
    agent::ticket_event(mem, &t, AgentEvent::Finished { summary }).expect("finished");
    settle(mem);
    exits(mem, &t, AgentRole::Worker, true);
}

/// One slot. A started and has a ship; then `edit` takes A to where it stands before the path,
/// with no agent; B took the slot and works.
fn b_works_after(edit: impl FnOnce(&mut Ticket)) -> Mem {
    let mut mem = with_agents();
    mem.project.max_parallel = 1;
    mem.view.edges.clear();
    mem.view.chain.autostart = false;
    recover(&mut mem).expect("start");
    let t = a(&mem);
    start(&mut mem, &t);
    assert_eq!(starts(&mem, &a(&mem), AgentRole::Worker), 1);
    edit(ticket_mut(&mut mem, "A"));
    mem.runner.live.borrow_mut().clear();
    let t = b(&mem);
    start(&mut mem, &t);
    assert_eq!(b(&mem).status, TicketStatus::Running, "B took the slot");
    assert_eq!(used(&mem), 1);
    mem.runner.started.borrow_mut().clear();
    mem
}

fn review(turn: ReviewTurn) -> impl FnOnce(&mut Ticket) {
    move |t| {
        t.status = TicketStatus::Review;
        t.status_reason = None;
        t.review.turn = Some(turn);
    }
}

fn blocked(reason: BlockedReason) -> impl FnOnce(&mut Ticket) {
    move |t| {
        t.status = TicketStatus::Blocked;
        t.status_reason = Some(StatusReason::Blocked {
            reason,
            resume_to: TicketStatus::Running,
            waiting: None,
        });
    }
}

fn check_done(passed: bool, mode: ReviewMode) -> TicketEvent {
    TicketEvent::CheckDone {
        passed,
        output: "cargo test: 1 failed".to_owned(),
        base: None,
        head: None,
        lines: Vec::new(),
        flaky: None,
        mode,
        max_rounds: 3,
    }
}

/// A path: B works, A gets there; A's agent of `role` waits with `words` and starts when B's
/// agent ends.
fn path_waits(name: &str, mut mem: Mem, go: impl FnOnce(&mut Mem), role: AgentRole, words: &str) {
    go(&mut mem);
    settle(&mut mem);
    let t = a(&mem);
    assert_eq!(starts(&mem, &t, role), 0, "{name}: no agent past the limit");
    let line = schedule::queue(&mem).expect("line");
    let (place, text) = line.get(&t.id).expect("A waits in line").clone();
    assert_eq!((place, text.as_str()), (1, words), "{name}");
    assert_eq!(
        b(&mem).status,
        TicketStatus::Running,
        "{name}: B is not stopped"
    );
    b_hands_in(&mut mem);
    let t = a(&mem);
    assert_eq!(starts(&mem, &t, role), 1, "{name}: A goes on when B leaves");
    assert!(!schedule::queue(&mem).expect("line").contains_key(&t.id));
    assert_eq!(used(&mem), 1, "{name}");
}

const SENT_BACK: &str = "sent back; goes on when an agent is free";
const UNBLOCKED: &str = "unblocked; goes on when an agent is free";
const REVIEWER: &str = "the reviewer is due; it starts when an agent is free";

#[test]
fn a_story_a_person_returns_waits_for_a_free_agent() {
    let mem = b_works_after(review(ReviewTurn::Person));
    let go = |mem: &mut Mem| {
        let comment = "rename the flag".to_owned();
        tickets::act(mem, &a(mem), TicketEvent::Return { comment }).expect("return");
    };
    path_waits("return", mem, go, AgentRole::Worker, SENT_BACK);
}

#[test]
fn a_story_the_check_sends_back_waits_for_a_free_agent() {
    let mem = b_works_after(review(ReviewTurn::Check));
    let go = |mem: &mut Mem| {
        let id = a(mem).id;
        tell_ticket(mem, id, check_done(false, ReviewMode::Human)).expect("check");
    };
    path_waits("check", mem, go, AgentRole::Worker, SENT_BACK);
}

#[test]
fn an_accepted_story_whose_re_check_failed_waits_for_a_free_agent() {
    let mem = b_works_after(|t| {
        review(ReviewTurn::Check)(t);
        t.review.accepted = true;
        t.review.recheck = true;
    });
    let go = |mem: &mut Mem| {
        let id = a(mem).id;
        tell_ticket(mem, id, check_done(false, ReviewMode::Human)).expect("re-check");
    };
    path_waits("re-check", mem, go, AgentRole::Worker, SENT_BACK);
}

#[test]
fn a_merge_conflict_handed_back_waits_for_a_free_agent() {
    let mem = b_works_after(review(ReviewTurn::Merging));
    let go = |mem: &mut Mem| {
        let id = a(mem).id;
        let text = "Conflicting files: a.rs.".to_owned();
        tell_ticket(mem, id, TicketEvent::MergeConflicted { text }).expect("conflict");
    };
    path_waits("conflict", mem, go, AgentRole::Worker, SENT_BACK);
}

#[test]
fn a_story_the_reviewer_sends_back_past_the_limit_waits_and_its_own_reviewer_does_not_count_for_it()
{
    let mem = b_works_after(review(ReviewTurn::Person));
    let go = |mem: &mut Mem| {
        // A's reviewer runs beside B, past the limit (the limit was lowered under it): it is
        // counted, and A's worker does not start in its place while B holds the only slot.
        let id = a(mem).id;
        ticket_mut(mem, "A").review.turn = Some(ReviewTurn::Agent { decides: true });
        mem.runner
            .live
            .borrow_mut()
            .insert((id, AgentRole::Reviewer));
        assert_eq!(used(mem), 2);
        let event = AgentEvent::ReviewVerdict {
            verdict: Verdict::RequestChanges,
            comment: "handle the empty list".to_owned(),
            items: Vec::new(),
        };
        agent::ticket_event(mem, &a(mem), event).expect("verdict");
    };
    path_waits("reviewer", mem, go, AgentRole::Worker, SENT_BACK);
}

#[test]
fn an_unblocked_story_waits_for_a_free_agent() {
    let text = "waiting for the key".to_owned();
    let mem = b_works_after(blocked(BlockedReason::Manual { text }));
    let go = |mem: &mut Mem| {
        tickets::act(mem, &a(mem), TicketEvent::Unblock).expect("unblock");
    };
    path_waits("unblock", mem, go, AgentRole::Worker, UNBLOCKED);
}

#[test]
fn a_start_despite_a_base_behind_waits_for_a_free_agent_and_keeps_the_decision() {
    for event in [TicketEvent::StartAnyway, TicketEvent::FastForwardStart] {
        let mem = b_works_after(blocked(BlockedReason::BaseBehind {
            base: "main".to_owned(),
            upstream: "origin/main".to_owned(),
            behind: 3,
            can_fast_forward: true,
        }));
        let name = event.name();
        let go = |mem: &mut Mem| {
            tickets::act(mem, &a(mem), event).expect("decision");
        };
        path_waits(name, mem, go, AgentRole::Worker, UNBLOCKED);
    }
}

#[test]
fn after_a_restart_the_replacements_start_one_per_free_slot_and_the_other_waits_its_turn() {
    let mut mem = with_agents();
    mem.view.edges.clear();
    recover(&mut mem).expect("start");
    settle(&mut mem);
    assert_eq!(used(&mem), 2, "two slots, two agents");
    mem.project.max_parallel = 1;
    // The daemon restarts: no process outlives it, and both stories want their agents back.
    mem.runner.live.borrow_mut().clear();
    mem.runner.started.borrow_mut().clear();
    mem.deferred.clear();
    let report = recover(&mut mem).expect("restart");
    assert_eq!(report.lost.len(), 2);
    settle(&mut mem);
    assert_eq!(starts(&mem, &a(&mem), AgentRole::Worker), 1, "A first");
    assert_eq!(starts(&mem, &b(&mem), AgentRole::Worker), 0, "B waits");
    let line = schedule::queue(&mem).expect("line");
    let words = "the app restarted; goes on when an agent is free";
    assert_eq!(line.get(&b(&mem).id), Some(&(1, words.to_owned())));
    assert_eq!(used(&mem), 1);
    // A hands in and its worker ends: B's replacement starts.
    let t = a(&mem);
    let summary = toknila_core::handin::from_plain("done");
    agent::ticket_event(&mut mem, &t, AgentEvent::Finished { summary }).expect("finished");
    settle(&mut mem);
    exits(&mut mem, &t, AgentRole::Worker, true);
    assert_eq!(starts(&mem, &b(&mem), AgentRole::Worker), 1);
    assert!(
        mem.runner.started.borrow()[1].brief.resume.is_some(),
        "B resumes"
    );
    assert_eq!(used(&mem), 1);
    let live: HashSet<_> = [b(&mem).id].into();
    assert!(
        runs::mark_lost(&mut mem, &live)
            .expect("nothing lost")
            .is_empty()
    );
}

#[test]
fn the_reviewer_after_a_hand_in_waits_for_a_free_agent() {
    let mem = b_works_after(review(ReviewTurn::Check));
    let go = |mem: &mut Mem| {
        let id = a(mem).id;
        tell_ticket(mem, id, check_done(true, ReviewMode::Agent)).expect("check");
    };
    path_waits("reviewer due", mem, go, AgentRole::Reviewer, REVIEWER);
}

#[test]
fn the_reviewer_a_person_runs_again_waits_for_a_free_agent() {
    let mem = b_works_after(|t| {
        review(ReviewTurn::Person)(t);
        t.review.note = Some(ReviewNote::NoVerdict { lines: Vec::new() });
    });
    let go = |mem: &mut Mem| {
        tickets::act(mem, &a(mem), TicketEvent::RunReviewer).expect("run reviewer");
    };
    path_waits("run reviewer", mem, go, AgentRole::Reviewer, REVIEWER);
}

// ---- each event that frees a slot starts the next story in line ----

/// One slot: B works and A, sent back by a person, waits first in line.
fn a_waits_for_b() -> Mem {
    let mut mem = b_works_after(review(ReviewTurn::Person));
    let comment = "again".to_owned();
    let t = a(&mem);
    tickets::act(&mut mem, &t, TicketEvent::Return { comment }).expect("return");
    settle(&mut mem);
    assert_eq!(starts(&mem, &a(&mem), AgentRole::Worker), 0);
    mem
}

fn a_went_on(mem: &Mem, event: &str) {
    let t = a(mem);
    assert_eq!(starts(mem, &t, AgentRole::Worker), 1, "{event} starts A");
    assert_eq!(t.agent_paused, None, "{event}");
    assert_eq!(used(mem), 1, "{event}");
}

#[test]
fn the_end_of_an_agents_process_starts_the_next_in_line() {
    let mut mem = a_waits_for_b();
    // B's agent crashes: B is blocked and its slot is free.
    let t = b(&mem);
    exits(&mut mem, &t, AgentRole::Worker, false);
    assert_eq!(b(&mem).status, TicketStatus::Blocked);
    a_went_on(&mem, "a crash");
}

#[test]
fn a_hand_in_frees_the_slot_once_the_worker_has_ended() {
    let mut mem = a_waits_for_b();
    let t = b(&mem);
    let summary = toknila_core::handin::from_plain("done");
    agent::ticket_event(&mut mem, &t, AgentEvent::Finished { summary }).expect("finished");
    // In the hand-in itself the worker is still alive (it is stopped after the commit), and it
    // still counts: no second agent beside it.
    assert!(!mem.deferred.iter().any(|e| matches!(
        e,
        crate::external::ExternalEffect::StartRunner { ticket, .. } if *ticket == a(&mem).id
    )));
    settle(&mut mem);
    exits(&mut mem, &t, AgentRole::Worker, true);
    a_went_on(&mem, "a hand-in");
}

#[test]
fn a_block_frees_the_slot() {
    let mut mem = a_waits_for_b();
    let reason = BlockedReason::Manual {
        text: "wait".to_owned(),
    };
    let t = b(&mem);
    tickets::act(&mut mem, &t, TicketEvent::Block { reason }).expect("block");
    settle(&mut mem);
    a_went_on(&mem, "a block");
}

#[test]
fn a_cancel_frees_the_slot() {
    let mut mem = a_waits_for_b();
    let reason = "not needed".to_owned();
    let t = b(&mem);
    tickets::act(&mut mem, &t, TicketEvent::Cancel { reason }).expect("cancel");
    settle(&mut mem);
    a_went_on(&mem, "a cancel");
}

#[test]
fn a_persons_pause_frees_the_slot() {
    let mut mem = a_waits_for_b();
    let t = b(&mem);
    tickets::act(&mut mem, &t, TicketEvent::Pause).expect("pause");
    settle(&mut mem);
    a_went_on(&mem, "a pause");
}

#[test]
fn an_agent_paused_while_it_waits_for_a_person_frees_the_slot() {
    let mut mem = a_waits_for_b();
    let ask = AgentEvent::AskHuman {
        text: "which db?".to_owned(),
        permission: false,
        detail: None,
        choices: Vec::new(),
    };
    let t = b(&mem);
    agent::ticket_event(&mut mem, &t, ask).expect("ask");
    settle(&mut mem);
    assert_eq!(
        starts(&mem, &a(&mem), AgentRole::Worker),
        0,
        "B still holds it"
    );
    let id = b(&mem).id;
    mem.runner
        .live
        .borrow_mut()
        .remove(&(id, AgentRole::Worker));
    paused::pause(&mut mem, id, Some("sess".to_owned())).expect("pause");
    settle(&mut mem);
    a_went_on(&mem, "an agent paused");
}

#[test]
fn a_raised_limit_starts_the_next_in_line_and_a_lowered_one_stops_nobody() {
    let mut mem = a_waits_for_b();
    let up = PatchProject {
        max_parallel: Some(2),
        ..PatchProject::default()
    };
    projects::patch(&mut mem, "DEMO", up).expect("raise");
    settle(&mut mem);
    a_went_on_beside_b(&mem);
    let down = PatchProject {
        max_parallel: Some(1),
        ..PatchProject::default()
    };
    projects::patch(&mut mem, "DEMO", down).expect("lower");
    settle(&mut mem);
    assert!(mem.runner.stopped.borrow().is_empty(), "nobody is stopped");
    assert_eq!(
        (a(&mem).status, b(&mem).status),
        (TicketStatus::Running, TicketStatus::Running)
    );
    assert_eq!(used(&mem), 2, "both counted past the limit");
}

fn a_went_on_beside_b(mem: &Mem) {
    assert_eq!(starts(mem, &a(mem), AgentRole::Worker), 1);
    assert_eq!(used(mem), 2);
}

// ---- a story never waits for itself ----

#[test]
fn with_one_slot_a_storys_whole_life_completes_and_the_next_waits_for_it() {
    let mut mem = with_agents();
    mem.project.max_parallel = 1;
    mem.project.default_review_mode = ReviewMode::Agent;
    mem.view.edges.clear();
    recover(&mut mem).expect("start");
    settle(&mut mem);
    assert_eq!(a(&mem).status, TicketStatus::Running);
    assert_eq!(b(&mem).status, TicketStatus::Ready, "B waits");
    for verdict in [Verdict::RequestChanges, Verdict::Approve] {
        // A hands in: its worker is still ending while its reviewer starts; one story, one slot.
        let summary = toknila_core::handin::from_plain("done");
        let t = a(&mem);
        agent::ticket_event(&mut mem, &t, AgentEvent::Finished { summary }).expect("finish");
        let reviewers = starts(&mem, &a(&mem), AgentRole::Reviewer);
        settle(&mut mem);
        assert_eq!(starts(&mem, &a(&mem), AgentRole::Reviewer), reviewers + 1);
        assert_eq!((used(&mem), b(&mem).status), (1, TicketStatus::Ready));
        let workers = starts(&mem, &a(&mem), AgentRole::Worker);
        let event = AgentEvent::ReviewVerdict {
            verdict,
            comment: "handle the empty list".to_owned(),
            items: Vec::new(),
        };
        let t = a(&mem);
        agent::ticket_event(&mut mem, &t, event).expect("verdict");
        settle(&mut mem);
        if verdict == Verdict::RequestChanges {
            // Sent back: the worker goes on in the slot its own reviewer held.
            assert_eq!(starts(&mem, &a(&mem), AgentRole::Worker), workers + 1);
            assert_eq!(a(&mem).status, TicketStatus::Running);
            assert_eq!((used(&mem), b(&mem).status), (1, TicketStatus::Ready));
        }
    }
    assert_eq!(a(&mem).status, TicketStatus::Merged);
    // A's last runner ends: B starts.
    let t = a(&mem);
    exits(&mut mem, &t, AgentRole::Reviewer, true);
    assert_eq!(b(&mem).status, TicketStatus::Running);
    assert_eq!(used(&mem), 1);
}
