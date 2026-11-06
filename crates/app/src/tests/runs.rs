//! Runners through the use cases. Like ships, a command only queues the process; `settle`
//! plays the adapter: run what was queued on a fake runner, then record the outcomes.

use std::cell::{Cell, RefCell};
use std::collections::HashSet;

use toknila_core::agent::AgentEvent;
use toknila_core::machine::TicketEvent;
use toknila_core::{
    Actor, BlockedReason, ProcessExit, Run, RunId, StatusReason, Ticket, TicketId, TicketStatus,
};

use super::Mem;
use crate::external::{self, ExternalEffect, Host};
use crate::recovery::recover;
use crate::runner::{RunBrief, RunEnded, RunRequest, Runner, RunnerError, RunnerState, Started};
use crate::{ErrorKind, agent, runs, tickets};
use toknila_core::instructions::terminal::Size;

mod delivery;

/// A runner that remembers what it was asked and can be told to refuse.
#[derive(Debug, Default)]
pub(super) struct FakeRunner {
    started: RefCell<Vec<RunRequest>>,
    stopped: RefCell<Vec<TicketId>>,
    written: RefCell<Vec<Vec<u8>>>,
    live: RefCell<HashSet<TicketId>>,
    refuse: RefCell<Option<RunnerError>>,
    sized: RefCell<Vec<Size>>,
    /// What `state` reports for a live ticket.
    reach: Cell<RunnerState>,
}

impl Default for RunnerState {
    fn default() -> Self {
        RunnerState {
            hooks: true,
            idle: false,
        }
    }
}

impl Runner for FakeRunner {
    fn start(&self, req: &RunRequest) -> Result<Started, RunnerError> {
        if let Some(e) = self.refuse.borrow().clone() {
            return Err(e);
        }
        let reused = !self.live.borrow_mut().insert(req.ticket);
        if !reused {
            self.started.borrow_mut().push(req.clone());
        }
        Ok(Started {
            run: run_of(req),
            reused,
        })
    }

    fn stop(&self, ticket: TicketId) {
        self.live.borrow_mut().remove(&ticket);
        self.stopped.borrow_mut().push(ticket);
    }

    fn write(&self, ticket: TicketId, bytes: &[u8]) -> Result<(), RunnerError> {
        if !self.live.borrow().contains(&ticket) {
            return Err(RunnerError::NotRunning);
        }
        self.written.borrow_mut().push(bytes.to_vec());
        Ok(())
    }

    fn state(&self, ticket: TicketId) -> Option<RunnerState> {
        self.live
            .borrow()
            .contains(&ticket)
            .then(|| self.reach.get())
    }

    fn resize(&self, ticket: TicketId, size: Size) -> Result<(), RunnerError> {
        if !self.live.borrow().contains(&ticket) {
            return Err(RunnerError::NotRunning);
        }
        self.sized.borrow_mut().push(size);
        Ok(())
    }
}

fn run_of(req: &RunRequest) -> Run {
    Run {
        id: RunId::new(),
        ticket_id: req.ticket,
        key: req.key.clone(),
        profile: req.profile.clone(),
        dir: req.dir.clone(),
    }
}

/// Project DEMO with a repository whose agents are processes (profile `fake`).
fn with_agents() -> Mem {
    let mut mem = Mem::demo();
    mem.project.repo_path = Some("/repo".to_owned());
    mem.project.default_profile = "fake".to_owned();
    mem
}

fn settle(mem: &mut Mem) {
    while !mem.deferred.is_empty() {
        let queued = std::mem::take(&mut mem.deferred);
        let host = Host {
            yard: &mem.yard,
            runner: &mem.runner,
        };
        let outcomes = external::execute(host, queued);
        external::apply(mem, outcomes).expect("apply outcomes");
    }
}

fn a(mem: &Mem) -> Ticket {
    mem.ticket_by_title("A")
}

fn blocked_reason(t: &Ticket) -> BlockedReason {
    let Some(StatusReason::Blocked { reason, .. }) = t.status_reason.clone() else {
        panic!("{} is not blocked: {:?}", t.key, t.status_reason);
    };
    reason
}

fn act(mem: &mut Mem, event: TicketEvent) {
    let t = a(mem);
    tickets::act(mem, &t, event).expect("act");
}

fn finish(mem: &mut Mem, summary: String) {
    let t = a(mem);
    agent::ticket_event(mem, &t, AgentEvent::Finished { summary }).expect("finished");
}

/// The host reports A's process ended.
fn end(mem: &mut Mem, code: Option<i32>, stopped: bool) {
    let e = ended(mem, code, stopped);
    runs::ended(mem, e).expect("ended");
}

fn ended(mem: &Mem, code: Option<i32>, stopped: bool) -> RunEnded {
    let t = a(mem);
    RunEnded {
        run: run_of(&RunRequest {
            ticket: t.id,
            key: t.key.clone(),
            profile: "fake".to_owned(),
            dir: "/ships/x".to_owned(),
            brief: RunBrief::default(),
        }),
        exit: ProcessExit { code, signal: None },
        stopped,
    }
}

#[test]
fn a_start_queues_the_runner_after_the_ship_and_runs_it_in_the_ship() {
    let mut mem = with_agents();
    recover(&mut mem).expect("settle");
    assert!(
        matches!(
            mem.deferred.as_slice(),
            [
                ExternalEffect::PrepareShip { .. },
                ExternalEffect::StartRunner { .. }
            ]
        ),
        "the ship first, then the runner: {:?}",
        mem.deferred
    );
    assert!(
        mem.runner.started.borrow().is_empty(),
        "not in the unit of work"
    );

    settle(&mut mem);
    let started = mem.runner.started.borrow().clone();
    let [req] = started.as_slice() else {
        panic!("one runner: {started:?}");
    };
    assert_eq!(req.dir, "/ships/demo/demo/demo-1-a");
    assert_eq!(req.profile, "fake");
    assert_eq!(
        mem.kinds()
            .iter()
            .filter(|k| *k == "ticket.runner_started")
            .count(),
        1
    );
    assert_eq!(a(&mem).status, TicketStatus::Running);
}

#[test]
fn a_simulated_ticket_queues_no_runner() {
    let mut mem = with_agents();
    mem.project.default_profile = "sim".to_owned();
    recover(&mut mem).expect("settle");
    assert!(
        mem.deferred
            .iter()
            .all(|e| !matches!(e, ExternalEffect::StartRunner { .. }))
    );
}

#[test]
fn a_ticket_profile_beats_the_project_default() {
    let mut mem = with_agents();
    mem.project.default_profile = "sim".to_owned();
    mem.view.tickets[0].profile = Some("fake".to_owned());
    recover(&mut mem).expect("settle");
    settle(&mut mem);
    assert_eq!(mem.runner.started.borrow().len(), 1);
}

#[test]
fn without_a_ship_there_is_no_directory_and_the_ticket_is_blocked_with_the_reason() {
    let mut mem = with_agents();
    mem.project.repo_path = None;
    recover(&mut mem).expect("settle");
    settle(&mut mem);
    let t = a(&mem);
    assert_eq!(t.status, TicketStatus::Blocked);
    assert_eq!(
        blocked_reason(&t),
        BlockedReason::Crashed {
            detail: RunnerError::NoWorkingDir.to_string()
        }
    );
    assert!(mem.runner.started.borrow().is_empty());
}

#[test]
fn a_failed_ship_starts_no_runner() {
    let mut mem = with_agents();
    *mem.yard.fail_prepare.borrow_mut() = Some(crate::ShipError::GitMissing);
    recover(&mut mem).expect("settle");
    settle(&mut mem);
    assert!(mem.runner.started.borrow().is_empty());
    assert!(matches!(
        blocked_reason(&a(&mem)),
        BlockedReason::ShipFailed { .. }
    ));
}

#[test]
fn a_runner_that_cannot_start_blocks_the_ticket_as_crashed() {
    let mut mem = with_agents();
    *mem.runner.refuse.borrow_mut() = Some(RunnerError::UnknownProfile("fake".to_owned()));
    recover(&mut mem).expect("settle");
    settle(&mut mem);
    assert_eq!(
        blocked_reason(&a(&mem)),
        BlockedReason::Crashed {
            detail: "agent profile fake does not exist".to_owned()
        }
    );
}

#[test]
fn cancel_and_accept_queue_a_stop_and_an_unchanged_runner_is_kept_on_return() {
    let mut mem = with_agents();
    recover(&mut mem).expect("settle");
    settle(&mut mem);
    let id = a(&mem).id;
    let summary = "done".to_owned();
    finish(&mut mem, summary);
    let comment = "again".to_owned();
    act(&mut mem, TicketEvent::Return { comment });
    settle(&mut mem);
    assert_eq!(
        mem.runner.started.borrow().len(),
        1,
        "the live runner is kept, not started twice"
    );
    assert_eq!(
        mem.kinds()
            .iter()
            .filter(|k| *k == "ticket.runner_started")
            .count(),
        1
    );

    let reason = "no".to_owned();
    act(&mut mem, TicketEvent::Cancel { reason });
    settle(&mut mem);
    assert_eq!(mem.runner.stopped.borrow().last(), Some(&id));
}

#[test]
fn a_crash_exit_blocks_the_ticket_with_the_code() {
    let mut mem = with_agents();
    recover(&mut mem).expect("settle");
    settle(&mut mem);
    end(&mut mem, Some(101), false);
    let t = a(&mem);
    assert_eq!(t.status, TicketStatus::Blocked);
    assert_eq!(
        blocked_reason(&t),
        BlockedReason::Crashed {
            detail: "exit 101".to_owned()
        }
    );
    assert_eq!(
        mem.kinds()
            .iter()
            .filter(|k| *k == "ticket.runner_exited")
            .count(),
        1
    );
}

#[test]
fn a_clean_exit_without_finished_blocks_the_ticket_and_unblock_starts_it_again() {
    let mut mem = with_agents();
    recover(&mut mem).expect("settle");
    settle(&mut mem);
    end(&mut mem, Some(0), false);
    assert_eq!(
        blocked_reason(&a(&mem)),
        BlockedReason::Crashed {
            detail: "exited without a finished event".to_owned()
        }
    );
    mem.runner.live.borrow_mut().clear();
    act(&mut mem, TicketEvent::Unblock);
    settle(&mut mem);
    assert_eq!(a(&mem).status, TicketStatus::Running);
    assert_eq!(mem.runner.started.borrow().len(), 2);
}

#[test]
fn an_exit_the_host_asked_for_is_journaled_and_blocks_nothing() {
    let mut mem = with_agents();
    recover(&mut mem).expect("settle");
    settle(&mut mem);
    end(&mut mem, None, true);
    assert_eq!(a(&mem).status, TicketStatus::Running);
    assert_eq!(
        mem.kinds()
            .iter()
            .filter(|k| *k == "ticket.runner_exited")
            .count(),
        1
    );
}

#[test]
fn an_exit_after_the_hand_in_changes_nothing() {
    let mut mem = with_agents();
    recover(&mut mem).expect("settle");
    settle(&mut mem);
    let summary = "done".to_owned();
    finish(&mut mem, summary);
    end(&mut mem, Some(0), false);
    assert_eq!(a(&mem).status, TicketStatus::Review);
}

#[test]
fn a_restart_blocks_working_process_tickets_as_runner_lost_and_leaves_sim_alone() {
    let mut mem = with_agents();
    mem.view.tickets[1].profile = Some("sim".to_owned());
    recover(&mut mem).expect("first start");
    settle(&mut mem);
    // The daemon died here: A's process is gone, and nothing was told.
    mem.runner.live.borrow_mut().clear();
    mem.deferred.clear();
    mem.events.clear();
    let report = recover(&mut mem).expect("restart");
    assert_eq!(report.lost, vec!["DEMO-1".to_owned()]);
    let t = a(&mem);
    assert_eq!(t.status, TicketStatus::Blocked);
    assert_eq!(blocked_reason(&t), BlockedReason::RunnerLost);
    assert!(
        mem.kinds().contains(&"ticket.status_changed".to_owned()),
        "journaled: {:?}",
        mem.kinds()
    );

    // Sim tickets have no process to lose.
    let mut mem = with_agents();
    mem.project.default_profile = "sim".to_owned();
    recover(&mut mem).expect("start");
    let report = recover(&mut mem).expect("restart");
    assert!(report.lost.is_empty());
    assert_eq!(a(&mem).status, TicketStatus::Running);
}

#[test]
fn only_a_person_writes_to_the_terminal_and_only_while_it_runs() {
    let mut mem = with_agents();
    recover(&mut mem).expect("settle");
    settle(&mut mem);
    let t = a(&mem);

    let refused = runs::send_input(&mem.runner, Actor::Agent, &t, b"x\n").expect_err("agent");
    assert_eq!(refused.kind, ErrorKind::Conflict);
    assert!(mem.runner.written.borrow().is_empty());

    runs::send_input(&mem.runner, Actor::Human, &t, b"yes\n").expect("person");
    assert_eq!(mem.runner.written.borrow().as_slice(), [b"yes\n".to_vec()]);

    mem.runner.live.borrow_mut().clear();
    let gone = runs::send_input(&mem.runner, Actor::Human, &t, b"yes\n").expect_err("no runner");
    assert_eq!(gone.kind, ErrorKind::Conflict);
}
