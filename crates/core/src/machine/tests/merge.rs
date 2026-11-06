//! Accept starts a merge on the host; the outcome comes back as an event.

use super::*;
use crate::{ReviewTurn, Verdict};

fn in_review(turn: ReviewTurn) -> Ticket {
    let mut t = ticket_in(TicketStatus::Review);
    t.review.turn = Some(turn);
    t
}

fn merging() -> Ticket {
    in_review(ReviewTurn::Merging)
}

fn step(t: &Ticket, event: &TicketEvent) -> Transition {
    let tr = apply(t, event, NOW).unwrap_or_else(|e| panic!("{event:?}: {e}"));
    check_invariants(&tr.ticket).unwrap_or_else(|e| panic!("{event:?}: {e}"));
    tr
}

fn conflict() -> BlockedReason {
    BlockedReason::MergeConflict {
        detail: "a.rs".to_owned(),
    }
}

#[test]
fn accept_starts_the_merge_and_the_card_stays_in_review() {
    let tr = step(
        &in_review(ReviewTurn::Person),
        &TicketEvent::Accept { comment: None },
    );
    assert_eq!(tr.ticket.status, TicketStatus::Review);
    assert_eq!(tr.ticket.review.turn, Some(ReviewTurn::Merging));
    assert_eq!(tr.effects, vec![Effect::Merge]);
    assert_eq!(tr.ticket.review.rounds[0].verdict, Verdict::Approve);
}

#[test]
fn an_agent_approval_that_decides_starts_the_merge_too() {
    let t = in_review(ReviewTurn::Agent { decides: true });
    let event = TicketEvent::ReviewVerdict {
        verdict: Verdict::Approve,
        comment: "ok".to_owned(),
    };
    let tr = step(&t, &event);
    assert_eq!(tr.ticket.status, TicketStatus::Review);
    assert_eq!(tr.ticket.review.turn, Some(ReviewTurn::Merging));
    assert!(tr.effects.contains(&Effect::Merge));
    assert!(!tr.effects.contains(&Effect::DependencySatisfied));
}

#[test]
fn a_merging_card_takes_neither_accept_nor_return_again() {
    for event in [
        TicketEvent::Accept { comment: None },
        TicketEvent::Return {
            comment: "x".to_owned(),
        },
    ] {
        assert!(
            matches!(
                apply(&merging(), &event, NOW),
                Err(TransitionError::NotAllowed { .. })
            ),
            "{event:?}"
        );
    }
}

#[test]
fn the_merge_outcome_merges_the_card_and_frees_the_ship_and_dependents() {
    let tr = step(&merging(), &TicketEvent::MergeDone);
    assert_eq!(tr.ticket.status, TicketStatus::Merged);
    assert_eq!(tr.ticket.review.turn, None);
    assert_eq!(
        tr.effects,
        vec![
            Effect::RemoveShip,
            Effect::StopRunner,
            Effect::DependencySatisfied
        ]
    );
}

#[test]
fn a_merge_outcome_for_a_card_that_is_not_merging_is_refused() {
    for t in [
        in_review(ReviewTurn::Person),
        ticket_in(TicketStatus::Running),
    ] {
        assert!(matches!(
            apply(&t, &TicketEvent::MergeDone, NOW),
            Err(TransitionError::NotAllowed { .. })
        ));
    }
}

#[test]
fn every_merge_failure_blocks_the_card_and_unblock_tries_the_merge_again() {
    let reasons = [
        conflict(),
        BlockedReason::BaseCheckoutDirty {
            path: "/repo".to_owned(),
        },
        BlockedReason::MergeFailed {
            detail: "too old".to_owned(),
        },
    ];
    for reason in reasons {
        let blocked = step(&merging(), &TicketEvent::Block { reason });
        assert_eq!(blocked.ticket.status, TicketStatus::Blocked);
        assert_eq!(blocked.ticket.review.turn, None);
        assert!(!blocked.effects.contains(&Effect::RemoveShip));
        assert!(!blocked.effects.contains(&Effect::StopRunner));
        let back = step(&blocked.ticket, &TicketEvent::Unblock);
        assert_eq!(back.ticket.status, TicketStatus::Review);
        assert_eq!(back.ticket.review.turn, Some(ReviewTurn::Merging));
        assert_eq!(back.effects, vec![Effect::Merge]);
    }
}

fn pull_request() -> crate::PullRequest {
    crate::PullRequest {
        number: 7,
        url: "https://github.com/o/r/pull/7".to_owned(),
        checks: vec![crate::CheckRun {
            name: "verify".to_owned(),
            state: crate::CheckState::Pending,
        }],
    }
}

#[test]
fn the_pull_request_is_kept_on_the_merging_card_without_a_status_change() {
    let tr = step(&merging(), &TicketEvent::PrUpdated { pr: pull_request() });
    assert_eq!(tr.ticket.status, TicketStatus::Review);
    assert_eq!(tr.ticket.review.turn, Some(ReviewTurn::Merging));
    assert_eq!(tr.ticket.pr, Some(pull_request()));
    assert!(tr.effects.is_empty());
}

#[test]
fn a_pull_request_update_for_a_card_that_is_not_merging_is_refused() {
    for t in [
        in_review(ReviewTurn::Person),
        ticket_in(TicketStatus::Running),
    ] {
        assert!(matches!(
            apply(&t, &TicketEvent::PrUpdated { pr: pull_request() }, NOW),
            Err(TransitionError::NotAllowed { .. })
        ));
    }
}

#[test]
fn failing_checks_and_a_closed_pull_request_block_the_card_and_unblock_merges_again() {
    let url = pull_request().url;
    for reason in [
        BlockedReason::CiFailed {
            url: url.clone(),
            checks: vec!["verify".to_owned()],
        },
        BlockedReason::PrClosed { url },
    ] {
        let blocked = step(&merging(), &TicketEvent::Block { reason });
        assert_eq!(blocked.ticket.status, TicketStatus::Blocked);
        let back = step(&blocked.ticket, &TicketEvent::Unblock);
        assert_eq!(back.ticket.review.turn, Some(ReviewTurn::Merging));
        assert_eq!(back.effects, vec![Effect::Merge]);
    }
}

#[test]
fn a_new_merge_forgets_the_old_pull_request_so_stale_checks_are_never_judged() {
    let mut blocked = merging();
    blocked.pr = Some(pull_request());
    let blocked = step(
        &blocked,
        &TicketEvent::Block {
            reason: BlockedReason::CiFailed {
                url: pull_request().url,
                checks: vec!["verify".to_owned()],
            },
        },
    );
    assert_eq!(
        blocked.ticket.pr,
        Some(pull_request()),
        "the card keeps its link"
    );
    let back = step(&blocked.ticket, &TicketEvent::Unblock);
    assert_eq!(back.ticket.pr, None);
}
