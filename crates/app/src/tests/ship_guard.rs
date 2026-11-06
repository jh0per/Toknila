//! The uncommitted-ship guard through the use cases, with a fake yard that says what the worktree
//! holds: Accept looks before the merger runs, and a ship with changes that are not committed or
//! with nothing of its own ends as a typed block while the merger is never asked.

use toknila_core::guard::ShipState;
use toknila_core::{BlockedReason, MergeMode, ReviewTurn, ShipLook, TicketStatus};

use super::Mem;
use super::merges::{accept, blocked_reason, in_review, unblock};
use super::runs::settle;
use crate::external::ExternalEffect;
use crate::ships::{self, GuardError};
use crate::{ShipError, ships::finished_look};

fn found(mem: &Mem, changed: &[&str], ahead: u32) {
    *mem.yard.found.borrow_mut() = Ok(ShipState {
        changed: changed.iter().map(|c| (*c).to_owned()).collect(),
        ahead,
    });
}

fn github() -> Mem {
    let mut mem = in_review();
    mem.project.merge.mode = MergeMode::Github;
    mem
}

#[test]
fn a_dirty_ship_blocks_the_accept_and_the_merger_is_never_asked() {
    let mut mem = in_review();
    found(&mem, &["src/a.rs", "notes.txt"], 2);
    accept(&mut mem);
    settle(&mut mem);
    let a = mem.ticket_by_title("A");
    assert_eq!(a.status, TicketStatus::Blocked);
    assert_eq!(
        blocked_reason(&a),
        BlockedReason::ShipUncommitted {
            files: vec!["src/a.rs".to_owned(), "notes.txt".to_owned()]
        }
    );
    assert!(mem.merger.asked.borrow().is_empty(), "nothing is merged");
    assert!(mem.yard.removed.borrow().is_empty(), "the ship stays");
    assert_eq!(mem.ticket_by_title("B").status, TicketStatus::Backlog);
}

#[test]
fn a_ship_with_nothing_ahead_blocks_the_accept_as_empty() {
    let mut mem = in_review();
    found(&mem, &[], 0);
    accept(&mut mem);
    settle(&mut mem);
    assert_eq!(
        blocked_reason(&mem.ticket_by_title("A")),
        BlockedReason::ShipEmpty
    );
    assert!(mem.merger.asked.borrow().is_empty());
}

#[test]
fn in_pull_request_mode_nothing_is_pushed_and_no_pull_request_is_opened() {
    // The merger is what pushes and opens the pull request; it is asked for neither.
    let mut mem = github();
    found(&mem, &["late.txt"], 1);
    accept(&mut mem);
    settle(&mut mem);
    assert!(matches!(
        blocked_reason(&mem.ticket_by_title("A")),
        BlockedReason::ShipUncommitted { .. }
    ));
    assert!(mem.merger.asked.borrow().is_empty());
    assert_eq!(mem.ticket_by_title("A").pr, None);
}

#[test]
fn the_ship_is_looked_at_where_the_worktree_is_against_where_it_started() {
    let mut local = in_review();
    accept(&mut local);
    settle(&mut local);
    assert_eq!(
        local.yard.looked.borrow().as_slice(),
        [ShipLook {
            path: "/ships/demo/demo/demo-1-a".to_owned(),
            start: "develop".to_owned()
        }]
    );
    let mut pr = github();
    accept(&mut pr);
    settle(&mut pr);
    assert_eq!(
        pr.yard.looked.borrow()[0].start,
        "origin/develop",
        "pull-request ships start from the remote base"
    );
}

#[test]
fn a_clean_ship_with_commits_merges_after_one_look() {
    let mut mem = in_review();
    accept(&mut mem);
    settle(&mut mem);
    assert_eq!(mem.ticket_by_title("A").status, TicketStatus::Merged);
    assert_eq!(mem.yard.looked.borrow().len(), 1);
    assert_eq!(mem.merger.asked.borrow().len(), 1);
}

#[test]
fn unblock_looks_again_and_merges_once_the_work_is_committed() {
    let mut mem = in_review();
    found(&mem, &["late.txt"], 1);
    accept(&mut mem);
    settle(&mut mem);
    assert_eq!(mem.ticket_by_title("A").status, TicketStatus::Blocked);

    found(&mem, &[], 2);
    unblock(&mut mem);
    let a = mem.ticket_by_title("A");
    assert_eq!(
        (a.status, a.review.turn),
        (TicketStatus::Review, Some(ReviewTurn::Merging))
    );
    settle(&mut mem);
    assert_eq!(mem.ticket_by_title("A").status, TicketStatus::Merged);
    assert_eq!(mem.yard.looked.borrow().len(), 2);
}

#[test]
fn a_pull_request_that_is_already_open_is_watched_without_looking_again() {
    let mut mem = github();
    super::merges::answer(
        &mem,
        Ok(crate::MergeOutcome::PrOpen {
            pr: toknila_core::PullRequest {
                number: 7,
                url: "https://example.test/pull/7".to_owned(),
                checks: Vec::new(),
            },
        }),
    );
    accept(&mut mem);
    settle(&mut mem);
    assert_eq!(
        mem.yard.looked.borrow().len(),
        1,
        "looked once, at the open"
    );
    // The worker edits a file while the checks run; the ticker only watches.
    found(&mem, &["scratch.txt"], 1);
    crate::merges::poll(&mut mem).expect("poll");
    settle(&mut mem);
    assert_eq!(mem.yard.looked.borrow().len(), 1);
    assert_eq!(mem.merger.asked.borrow().len(), 2);
    assert_eq!(mem.ticket_by_title("A").status, TicketStatus::Review);
}

#[test]
fn a_yard_that_cannot_look_fails_the_merge_with_the_reason() {
    let mut mem = in_review();
    *mem.yard.found.borrow_mut() = Err(ShipError::GitMissing);
    accept(&mut mem);
    settle(&mut mem);
    let BlockedReason::MergeFailed { detail } = blocked_reason(&mem.ticket_by_title("A")) else {
        panic!("merge_failed");
    };
    assert!(detail.contains("git is not installed"), "{detail}");
    assert!(mem.merger.asked.borrow().is_empty());
}

#[test]
fn the_look_for_finished_names_the_worktree_and_start_and_is_none_without_a_ship() {
    let mem = in_review();
    let a = mem.ticket_by_title("A");
    let look = finished_look(&mem, &a).expect("look").expect("a ship");
    assert_eq!(look.path, "/ships/demo/demo/demo-1-a");
    assert_eq!(look.start, "develop");
    let b = mem.ticket_by_title("B");
    assert_eq!(
        finished_look(&mem, &b).expect("look"),
        None,
        "B has no ship yet"
    );
}

#[test]
fn the_yard_decides_finished_and_the_card_is_told_what_to_do() {
    let mem = in_review();
    let look = ShipLook {
        path: "/ships/x".to_owned(),
        start: "develop".to_owned(),
    };
    found(&mem, &["a.rs"], 3);
    let err = ships::guard(&mem.yard, &look).expect_err("dirty");
    let refused = ships::refuse_finished(err, &look);
    assert_eq!(refused.kind, crate::ErrorKind::Conflict);
    assert!(refused.to_string().contains("git commit"), "{refused}");

    found(&mem, &[], 0);
    assert!(matches!(
        ships::guard(&mem.yard, &look),
        Err(GuardError::Problem(_))
    ));
    found(&mem, &[], 1);
    assert_eq!(ships::guard(&mem.yard, &look), Ok(()));
    assert!(
        !mem.deferred
            .iter()
            .any(|e| matches!(e, ExternalEffect::Merge { .. }))
    );
}
