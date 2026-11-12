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
use crate::{MergeError, MergeOutcome, Merger, SyncOutcome, SyncSpec, agent, tickets};

/// A merger that remembers what it was asked and answers what it was told to.
#[derive(Debug)]
pub(super) struct FakeMerger {
    pub(super) asked: RefCell<Vec<MergeSpec>>,
    answer: RefCell<Result<MergeOutcome, MergeError>>,
    /// What bringing a ship up to its base was asked, and what it answers.
    pub(super) synced: RefCell<Vec<SyncSpec>>,
    pub(super) sync_answer: RefCell<Result<SyncOutcome, MergeError>>,
}

impl Default for FakeMerger {
    fn default() -> Self {
        Self {
            asked: RefCell::default(),
            synced: RefCell::default(),
            sync_answer: RefCell::new(Ok(SyncOutcome::UpToDate)),
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

    fn sync(&self, spec: &SyncSpec) -> Result<SyncOutcome, MergeError> {
        self.synced.borrow_mut().push(spec.clone());
        self.sync_answer.borrow().clone()
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
    let summary = toknila_core::handin::from_plain("done");
    agent::ticket_event(&mut mem, &a, AgentEvent::Finished { summary }).expect("finished");
    // The hand-in brings the ship up to its base first (no check command: only the sync runs);
    // what the tests below count starts after it.
    settle(&mut mem);
    mem.merger.synced.borrow_mut().clear();
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
fn a_conflict_returns_the_card_to_its_agent_with_the_files_and_keeps_the_ship_and_dependents_waiting()
 {
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
    assert_eq!(a.status, TicketStatus::Running);
    assert!(a.review.accepted, "the person's Accept is remembered");
    let text = returned_text(&mem);
    assert!(
        text.contains("src/a.rs") && text.contains("b.txt") && text.contains("git merge develop"),
        "{text}"
    );
    assert!(mem.yard.removed.borrow().is_empty());
    assert_eq!(mem.ticket_by_title("B").status, TicketStatus::Backlog);
}

/// The text of the last instruction the system sent the worker.
fn returned_text(mem: &Mem) -> String {
    let c = mem
        .comments
        .iter()
        .rfind(|c| c.kind == toknila_core::CommentKind::Instruction)
        .expect("an instruction was sent");
    assert_eq!(c.author, toknila_core::Actor::System);
    format!(
        "{} {}",
        c.body.gist,
        c.body.what.clone().unwrap_or_default()
    )
}

#[test]
fn after_a_conflict_at_accept_the_next_hand_in_merges_without_a_second_accept() {
    let mut mem = in_review();
    *mem.merger.sync_answer.borrow_mut() = Ok(SyncOutcome::Conflict {
        files: vec!["a.rs".to_owned()],
        migrations: Vec::new(),
    });
    accept(&mut mem);
    settle(&mut mem);
    assert_eq!(mem.ticket_by_title("A").status, TicketStatus::Running);
    // Return is not needed and not possible: the card is already with its agent.
    *mem.merger.sync_answer.borrow_mut() = Ok(SyncOutcome::UpToDate);
    let a = mem.ticket_by_title("A");
    let summary = toknila_core::handin::from_plain("merged main, resolved");
    agent::ticket_event(&mut mem, &a, AgentEvent::Finished { summary }).expect("finished");
    settle(&mut mem);
    let a = mem.ticket_by_title("A");
    assert_eq!(a.status, TicketStatus::Merged, "no second Accept");
    assert!(!a.review.accepted);
    assert_eq!(mem.merger.asked.borrow().len(), 1);
}

#[test]
fn a_hand_in_that_still_conflicts_does_not_merge_and_a_person_return_forgets_the_accept() {
    let mut mem = in_review();
    *mem.merger.sync_answer.borrow_mut() = Ok(SyncOutcome::Conflict {
        files: vec!["a.rs".to_owned()],
        migrations: Vec::new(),
    });
    accept(&mut mem);
    settle(&mut mem);
    let a = mem.ticket_by_title("A");
    let summary = toknila_core::handin::from_plain("again");
    agent::ticket_event(&mut mem, &a, AgentEvent::Finished { summary }).expect("finished");
    settle(&mut mem);
    let a = mem.ticket_by_title("A");
    assert_eq!(a.status, TicketStatus::Running, "back again, not merged");
    assert!(mem.merger.asked.borrow().is_empty());
    assert!(a.review.accepted);
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
    let summary = toknila_core::handin::from_plain("done");
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

#[test]
fn accept_on_a_ship_that_fell_behind_syncs_first_and_a_conflict_returns_the_card_with_the_files() {
    let mut mem = in_review();
    *mem.merger.sync_answer.borrow_mut() = Ok(SyncOutcome::Conflict {
        files: vec!["src/lib.rs".to_owned()],
        migrations: Vec::new(),
    });
    accept(&mut mem);
    settle(&mut mem);
    let a = mem.ticket_by_title("A");
    assert_eq!(mem.merger.synced.borrow().len(), 1);
    assert!(
        mem.merger.asked.borrow().is_empty(),
        "the merge did not run"
    );
    assert_eq!(a.status, TicketStatus::Running);
    let text = returned_text(&mem);
    assert!(text.contains("Conflicting files: src/lib.rs."), "{text}");
    assert!(
        text.contains("git merge develop, resolve it, run the gates")
            || text.contains("`git merge develop` in the ship, resolve it, run the gates"),
        "{text}"
    );
}

#[test]
fn a_conflict_on_a_card_without_a_ship_still_blocks_as_merge_conflict() {
    // The sim profile: no ship, so nobody to return the card to.
    let mut mem = Mem::demo();
    recover(&mut mem).expect("settle");
    let a = mem.ticket_by_title("A");
    let summary = toknila_core::handin::from_plain("done");
    agent::ticket_event(&mut mem, &a, AgentEvent::Finished { summary }).expect("finished");
    accept(&mut mem);
    let a = mem.ticket_by_title("A");
    crate::merges::finished(
        &mut mem,
        a.id,
        Ok(MergeOutcome::Conflict {
            files: vec!["x".to_owned()],
        }),
    )
    .expect("outcome");
    let BlockedReason::MergeConflict { detail } = blocked_reason(&mem.ticket_by_title("A")) else {
        panic!("a merge conflict");
    };
    assert!(detail.contains('x'), "{detail}");
}

#[test]
fn accept_syncs_the_ship_before_the_merge() {
    let mut mem = in_review();
    accept(&mut mem);
    settle(&mut mem);
    assert_eq!(mem.merger.synced.borrow().len(), 1);
    assert_eq!(mem.merger.asked.borrow().len(), 1);
}

#[test]
fn a_merged_card_in_local_mode_retires_its_ship_and_a_kept_branch_is_explained() {
    for (fate, comments) in [
        (crate::BranchFate::Deleted, 0),
        (crate::BranchFate::Kept { ahead: 2 }, 1),
    ] {
        let mut mem = in_review();
        *mem.yard.fate.borrow_mut() = fate;
        accept(&mut mem);
        settle(&mut mem);
        let a = mem.ticket_by_title("A");
        assert_eq!(a.status, TicketStatus::Merged);
        assert_eq!(mem.yard.removed.borrow().len(), 1);
        assert_eq!(
            *mem.yard.retired.borrow(),
            ["develop"],
            "measured against the base"
        );
        let notes: Vec<_> = mem
            .comments
            .iter()
            .filter(|c| c.ticket_id == a.id && c.author == toknila_core::Actor::System)
            .collect();
        assert_eq!(notes.len(), comments, "{notes:?}");
        if let Some(n) = notes.first() {
            assert!(
                n.body
                    .what
                    .as_deref()
                    .is_some_and(|w| w.contains("the merge"))
            );
        }
    }
}

#[test]
fn a_merged_card_in_pull_request_mode_leaves_its_branch_to_the_forge() {
    let mut mem = in_review();
    mem.project.merge.mode = toknila_core::MergeMode::Github;
    *mem.yard.fate.borrow_mut() = crate::BranchFate::Deleted;
    accept(&mut mem);
    settle(&mut mem);
    let a = mem.ticket_by_title("A");
    assert_eq!(a.status, TicketStatus::Merged);
    assert_eq!(mem.yard.removed.borrow().len(), 1, "the worktree goes");
    assert!(
        mem.yard.retired.borrow().is_empty(),
        "no branch is looked at or deleted"
    );
}
