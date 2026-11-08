//! Accept through the use cases: the merge is queued for after the commit and its outcome moves
//! the card (`Merged`, or `Blocked` with a typed reason).

use std::cell::RefCell;

use toknila_core::agent::AgentEvent;
use toknila_core::machine::TicketEvent;
use toknila_core::{BlockedReason, MergeSpec, ReviewTurn, StatusReason, Ticket, TicketStatus};

use super::Mem;
use super::runs::settle;
use crate::GitError;
use crate::external::ExternalEffect;
use crate::recovery::recover;
use crate::{MergeError, MergeOutcome, Merger, agent, tickets};

/// A merger that remembers what it was asked and answers what it was told to.
#[derive(Debug)]
pub(super) struct FakeMerger {
    pub(super) asked: RefCell<Vec<MergeSpec>>,
    answer: RefCell<Result<MergeOutcome, MergeError>>,
}

impl Default for FakeMerger {
    fn default() -> Self {
        Self {
            asked: RefCell::default(),
            answer: RefCell::new(Ok(MergeOutcome::Merged {
                commit: "abc".to_owned(),
            })),
        }
    }
}

impl Merger for FakeMerger {
    fn merge(&self, spec: &MergeSpec) -> Result<MergeOutcome, MergeError> {
        self.asked.borrow_mut().push(spec.clone());
        self.answer.borrow().clone()
    }
}

pub(super) fn answer(mem: &Mem, answer: Result<MergeOutcome, MergeError>) {
    *mem.merger.answer.borrow_mut() = answer;
}

/// DEMO-1 (A) is in Review with its ship made; DEMO-2 (B) waits for it.
pub(super) fn in_review() -> Mem {
    let mut mem = Mem::demo();
    mem.project.repo_path = Some("/repo".to_owned());
    recover(&mut mem).expect("settle");
    settle(&mut mem);
    let a = mem.ticket_by_title("A");
    let summary = "done".to_owned();
    agent::ticket_event(&mut mem, &a, AgentEvent::Finished { summary }).expect("finished");
    mem
}

pub(super) fn accept(mem: &mut Mem) {
    let a = mem.ticket_by_title("A");
    tickets::act(mem, &a, TicketEvent::Accept { comment: None }).expect("accept");
}

pub(super) fn unblock(mem: &mut Mem) {
    let a = mem.ticket_by_title("A");
    tickets::act(mem, &a, TicketEvent::Unblock).expect("unblock");
}

pub(super) fn blocked_reason(t: &Ticket) -> BlockedReason {
    let Some(StatusReason::Blocked { reason, .. }) = t.status_reason.clone() else {
        panic!("not blocked: {:?}", t.status_reason);
    };
    reason
}

#[test]
fn accept_queues_the_merge_and_the_card_waits_in_review_for_the_outcome() {
    let mut mem = in_review();
    accept(&mut mem);
    let a = mem.ticket_by_title("A");
    assert_eq!(a.status, TicketStatus::Review);
    assert_eq!(a.review.turn, Some(ReviewTurn::Merging));
    assert!(
        mem.merger.asked.borrow().is_empty(),
        "git does not run inside the unit of work"
    );
    let Some(ExternalEffect::Merge {
        spec: Some(spec), ..
    }) = mem
        .deferred
        .iter()
        .find(|e| matches!(e, ExternalEffect::Merge { .. }))
    else {
        panic!("a merge is queued: {:?}", mem.deferred);
    };
    assert_eq!(
        (spec.repo.as_str(), spec.base.as_str(), spec.branch.as_str()),
        ("/repo", "develop", "ship/demo/demo-1-a")
    );
    assert_eq!(spec.message, "Merge ship/demo/demo-1-a: DEMO-1 A");
    assert!(
        !mem.deferred
            .iter()
            .any(|e| matches!(e, ExternalEffect::RemoveShip { .. })),
        "the ship stays until the merge landed"
    );
}

#[test]
fn a_merge_that_lands_merges_the_card_removes_the_ship_and_starts_dependents() {
    let mut mem = in_review();
    accept(&mut mem);
    settle(&mut mem);
    assert_eq!(mem.ticket_by_title("A").status, TicketStatus::Merged);
    assert_eq!(mem.merger.asked.borrow().len(), 1);
    assert_eq!(
        mem.yard.removed.borrow().len(),
        1,
        "the worktree is removed"
    );
    assert_eq!(mem.ticket_by_title("B").status, TicketStatus::Running);
}

#[test]
fn a_conflict_blocks_the_card_with_the_files_and_keeps_the_ship_and_dependents_waiting() {
    let mut mem = in_review();
    answer(
        &mem,
        Ok(MergeOutcome::Conflict {
            files: vec!["src/a.rs".to_owned(), "b.txt".to_owned()],
        }),
    );
    accept(&mut mem);
    settle(&mut mem);
    let a = mem.ticket_by_title("A");
    assert_eq!(a.status, TicketStatus::Blocked);
    let BlockedReason::MergeConflict { detail } = blocked_reason(&a) else {
        panic!("a merge conflict");
    };
    assert!(
        detail.contains("src/a.rs") && detail.contains("b.txt"),
        "{detail}"
    );
    assert!(mem.yard.removed.borrow().is_empty());
    assert_eq!(mem.ticket_by_title("B").status, TicketStatus::Backlog);
}

#[test]
fn unblock_after_a_conflict_merges_again() {
    let mut mem = in_review();
    answer(
        &mem,
        Ok(MergeOutcome::Conflict {
            files: vec!["a.rs".to_owned()],
        }),
    );
    accept(&mut mem);
    settle(&mut mem);
    answer(&mem, Ok(MergeOutcome::Merged { commit: "d".into() }));
    unblock(&mut mem);
    let a = mem.ticket_by_title("A");
    assert_eq!(
        (a.status, a.review.turn),
        (TicketStatus::Review, Some(ReviewTurn::Merging))
    );
    settle(&mut mem);
    assert_eq!(mem.ticket_by_title("A").status, TicketStatus::Merged);
    assert_eq!(mem.merger.asked.borrow().len(), 2);
}

#[test]
fn a_dirty_base_checkout_and_a_failed_merge_block_with_their_own_reasons() {
    let mut mem = in_review();
    answer(
        &mem,
        Ok(MergeOutcome::BaseCheckoutDirty {
            path: "/repo".to_owned(),
        }),
    );
    accept(&mut mem);
    settle(&mut mem);
    assert_eq!(
        blocked_reason(&mem.ticket_by_title("A")),
        BlockedReason::BaseCheckoutDirty {
            path: "/repo".to_owned()
        }
    );

    answer(&mem, Err(MergeError::Git(GitError::Missing)));
    unblock(&mut mem);
    settle(&mut mem);
    assert_eq!(
        blocked_reason(&mem.ticket_by_title("A")),
        BlockedReason::MergeFailed {
            detail: "git is not installed or not on PATH".to_owned()
        }
    );
}

#[test]
fn a_project_without_a_repository_has_nothing_to_merge_and_the_card_is_merged() {
    let mut mem = Mem::demo();
    recover(&mut mem).expect("settle");
    let a = mem.ticket_by_title("A");
    let summary = "done".to_owned();
    agent::ticket_event(&mut mem, &a, AgentEvent::Finished { summary }).expect("finished");
    accept(&mut mem);
    assert_eq!(mem.ticket_by_title("A").status, TicketStatus::Review);
    settle(&mut mem);
    assert_eq!(mem.ticket_by_title("A").status, TicketStatus::Merged);
    assert!(mem.merger.asked.borrow().is_empty());
}

#[test]
fn a_merge_running_at_a_restart_is_queued_again() {
    let mut mem = in_review();
    accept(&mut mem);
    mem.deferred.clear(); // the daemon stopped before the host ran it
    let report = recover(&mut mem).expect("recover");
    assert_eq!(report.merges_resumed, ["DEMO-1"]);
    settle(&mut mem);
    assert_eq!(mem.ticket_by_title("A").status, TicketStatus::Merged);
}
