//! Ships through the use cases. A command only queues the work (git is not run inside its unit
//! of work); `settle_ships` plays the adapter: run the queued work, then record the outcomes.

use std::cell::RefCell;

use toknila_core::agent::AgentEvent;
use toknila_core::machine::TicketEvent;
use toknila_core::{BlockedReason, Ship, ShipSpec, StatusReason, TicketStatus};

use super::Mem;
use crate::external::{self, ExternalEffect, Host};
use crate::recovery::recover;
use crate::{AppResult, ShipError, Shipyard, agent, tickets};

/// A shipyard that remembers what it was asked and can be told to fail.
#[derive(Debug, Default)]
pub(super) struct FakeYard {
    prepared: RefCell<Vec<ShipSpec>>,
    removed: RefCell<Vec<Ship>>,
    pub(super) fail_prepare: RefCell<Option<ShipError>>,
    fail_remove: RefCell<Option<ShipError>>,
}

impl Shipyard for FakeYard {
    fn prepare(&self, spec: &ShipSpec) -> Result<String, ShipError> {
        if let Some(e) = self.fail_prepare.borrow().clone() {
            return Err(e);
        }
        self.prepared.borrow_mut().push(spec.clone());
        Ok(format!(
            "/ships/{}/{}/{}",
            spec.project, spec.chain, spec.slug
        ))
    }

    fn remove(&self, _repo: &str, ship: &Ship) -> Result<(), ShipError> {
        if let Some(e) = self.fail_remove.borrow().clone() {
            return Err(e);
        }
        self.removed.borrow_mut().push(ship.clone());
        Ok(())
    }
}

/// Project DEMO with a repository; nothing has started yet.
fn with_repo() -> Mem {
    let mut mem = Mem::demo();
    mem.project.repo_path = Some("/repo".to_owned());
    mem
}

/// What the adapter does after a command commits: run what was queued, record the outcomes.
fn settle_ships(mem: &mut Mem) {
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

fn finish_a(mem: &mut Mem) -> AppResult<()> {
    let a = mem.ticket_by_title("A");
    let summary = "done".to_owned();
    agent::ticket_event(mem, &a, AgentEvent::Finished { summary }).map(|_| ())
}

fn act_on_a(mem: &mut Mem, event: TicketEvent) -> AppResult<()> {
    let a = mem.ticket_by_title("A");
    tickets::act(mem, &a, event)
}

fn count(mem: &Mem, kind: &str) -> usize {
    mem.kinds().iter().filter(|k| *k == kind).count()
}

#[test]
fn starting_a_ticket_only_queues_the_ship_and_the_outcome_makes_it() {
    let mut mem = with_repo();
    recover(&mut mem).expect("settle");
    assert_eq!(mem.ticket_by_title("A").status, TicketStatus::Running);
    assert!(
        mem.yard.prepared.borrow().is_empty(),
        "git does not run inside the unit of work"
    );
    assert!(mem.ships.is_empty());
    let [ExternalEffect::PrepareShip { spec, .. }] = mem.deferred.as_slice() else {
        panic!("one queued ship: {:?}", mem.deferred);
    };
    assert_eq!(
        (spec.repo.as_str(), spec.base.as_str(), spec.branch.as_str()),
        ("/repo", "develop", "ship/demo/demo-1-a")
    );

    settle_ships(&mut mem);
    let a = mem.ticket_by_title("A");
    let ship = mem.ships.first().expect("ship").clone();
    assert_eq!(ship.branch, "ship/demo/demo-1-a");
    assert_eq!(ship.path, "/ships/demo/demo/demo-1-a");
    assert_eq!(a.branch.as_deref(), Some("ship/demo/demo-1-a"));
    assert_eq!(a.status, TicketStatus::Running);
    assert_eq!(count(&mem, "ticket.ship_created"), 1);
    assert_eq!(mem.ships.len(), 1, "B waits for A and has no ship yet");
}

#[test]
fn a_project_without_a_repository_queues_nothing() {
    let mut mem = Mem::demo();
    recover(&mut mem).expect("settle");
    assert_eq!(mem.ticket_by_title("A").status, TicketStatus::Running);
    assert!(mem.deferred.is_empty());
    assert!(mem.ships.is_empty());
}

#[test]
fn merged_queues_the_removal_and_the_outcome_forgets_the_path_but_keeps_the_branch() {
    let mut mem = with_repo();
    recover(&mut mem).expect("settle");
    settle_ships(&mut mem);
    finish_a(&mut mem).expect("finished");
    act_on_a(&mut mem, TicketEvent::Accept { comment: None }).expect("accept");
    assert_eq!(mem.ticket_by_title("A").status, TicketStatus::Merged);
    assert!(
        mem.deferred
            .iter()
            .any(|e| matches!(e, ExternalEffect::RemoveShip { .. }))
    );
    assert!(mem.yard.removed.borrow().is_empty(), "not yet");

    settle_ships(&mut mem);
    assert_eq!(mem.yard.removed.borrow().len(), 1);
    assert!(mem.ships.iter().all(|s| s.key.to_string() != "DEMO-1"));
    assert_eq!(count(&mem, "ticket.ship_removed"), 1);
    let branch = mem.ticket_by_title("A").branch;
    assert_eq!(branch.as_deref(), Some("ship/demo/demo-1-a"));
}

#[test]
fn a_removal_that_fails_is_journaled_and_the_accept_stands() {
    let mut mem = with_repo();
    recover(&mut mem).expect("settle");
    settle_ships(&mut mem);
    finish_a(&mut mem).expect("finished");
    *mem.yard.fail_remove.borrow_mut() = Some(ShipError::Io("locked".to_owned()));
    act_on_a(&mut mem, TicketEvent::Accept { comment: None }).expect("accept");
    settle_ships(&mut mem);
    assert_eq!(mem.ticket_by_title("A").status, TicketStatus::Merged);
    assert_eq!(count(&mem, "ticket.ship_remove_failed"), 1);
    assert_eq!(count(&mem, "ticket.ship_removed"), 0);
    assert!(
        mem.ships.iter().any(|s| s.key.to_string() == "DEMO-1"),
        "the path is still recorded"
    );
}

#[test]
fn a_returned_ticket_reuses_its_ship() {
    let mut mem = with_repo();
    recover(&mut mem).expect("settle");
    settle_ships(&mut mem);
    finish_a(&mut mem).expect("finished");
    let comment = "add a test".to_owned();
    act_on_a(&mut mem, TicketEvent::Return { comment }).expect("return");
    settle_ships(&mut mem);
    assert_eq!(mem.ticket_by_title("A").status, TicketStatus::Running);
    assert_eq!(
        mem.yard.prepared.borrow().len(),
        2,
        "the yard is asked again"
    );
    assert_eq!(mem.ships.len(), 1);
    assert_eq!(
        count(&mem, "ticket.ship_created"),
        1,
        "but nothing new is journaled"
    );
}

#[test]
fn a_failed_ship_blocks_the_ticket_with_the_reason_and_unblock_retries() {
    let mut mem = with_repo();
    *mem.yard.fail_prepare.borrow_mut() = Some(ShipError::GitMissing);
    recover(&mut mem).expect("settle");
    assert_eq!(mem.ticket_by_title("A").status, TicketStatus::Running);
    settle_ships(&mut mem);
    let a = mem.ticket_by_title("A");
    assert_eq!(a.status, TicketStatus::Blocked);
    let Some(StatusReason::Blocked { reason, .. }) = a.status_reason else {
        panic!("blocked ticket has a blocked reason");
    };
    assert_eq!(
        reason,
        BlockedReason::ShipFailed {
            detail: "git is not installed or not on PATH".to_owned()
        }
    );
    assert!(mem.ships.is_empty());
    assert_eq!(
        mem.ticket_by_title("B").status,
        TicketStatus::Backlog,
        "dependents keep waiting"
    );

    *mem.yard.fail_prepare.borrow_mut() = None;
    act_on_a(&mut mem, TicketEvent::Unblock).expect("unblock");
    settle_ships(&mut mem);
    assert_eq!(mem.ticket_by_title("A").status, TicketStatus::Running);
    assert_eq!(mem.ships.len(), 1);
}

#[test]
fn an_outcome_for_a_ticket_that_moved_on_does_not_fail() {
    let mut mem = with_repo();
    *mem.yard.fail_prepare.borrow_mut() = Some(ShipError::GitMissing);
    recover(&mut mem).expect("settle");
    let reason = "not needed".to_owned();
    act_on_a(&mut mem, TicketEvent::Cancel { reason }).expect("cancel");
    settle_ships(&mut mem);
    assert_eq!(mem.ticket_by_title("A").status, TicketStatus::Cancelled);
}

#[test]
fn cancel_keeps_worktree_and_branch() {
    let mut mem = with_repo();
    recover(&mut mem).expect("settle");
    settle_ships(&mut mem);
    let reason = "not needed".to_owned();
    act_on_a(&mut mem, TicketEvent::Cancel { reason }).expect("cancel");
    settle_ships(&mut mem);
    assert_eq!(mem.ticket_by_title("A").status, TicketStatus::Cancelled);
    let a = mem.ticket_by_title("A");
    assert!(mem.ships.iter().any(|s| s.ticket_id == a.id));
    assert!(mem.yard.removed.borrow().is_empty());
}
