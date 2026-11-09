//! A pull request that reports no check: after the timeout a person is asked (merge now, keep
//! waiting, block); a pull request with checks is never asked.

use super::*;
use crate::{CheckRun, CheckState, PullRequest, ReviewNote, ReviewTurn};

const URL: &str = "https://github.com/o/r/pull/7";
const TIMEOUT: u64 = 600;

fn merging() -> Ticket {
    let mut t = ticket_in(TicketStatus::Review);
    t.review.turn = Some(ReviewTurn::Merging);
    t
}

fn look(t: &Ticket, checks: Vec<CheckRun>, at_secs: i64) -> Ticket {
    let event = TicketEvent::PrUpdated {
        pr: PullRequest::seen(7, URL.to_owned(), checks),
        ask_after_secs: TIMEOUT,
    };
    let tr = apply(t, &event, Timestamp(at_secs * 1000)).expect("pr update");
    check_invariants(&tr.ticket).expect("invariants");
    tr.ticket
}

fn check(state: CheckState) -> CheckRun {
    CheckRun {
        name: "verify".to_owned(),
        state,
    }
}

/// A card that has seen no check since second 0 and was asked at the timeout.
fn asked() -> Ticket {
    let t = look(&merging(), Vec::new(), 0);
    look(&t, Vec::new(), TIMEOUT as i64)
}

#[test]
fn no_check_before_the_timeout_asks_nobody_and_at_the_timeout_asks() {
    let t = look(&merging(), Vec::new(), 0);
    assert_eq!(
        t.pr.as_ref().and_then(|p| p.no_checks_since),
        Some(Timestamp(0))
    );
    let t = look(&t, Vec::new(), TIMEOUT as i64 - 1);
    assert_eq!(t.review.note, None);
    let t = look(&t, Vec::new(), TIMEOUT as i64);
    assert_eq!(t.review.note, Some(ReviewNote::NoChecks));
    // The timer is the first look without checks, not the latest.
    assert_eq!(
        t.pr.as_ref().and_then(|p| p.no_checks_since),
        Some(Timestamp(0))
    );
}

#[test]
fn a_pull_request_with_checks_is_never_asked_about() {
    for state in [CheckState::Pending, CheckState::Passed, CheckState::Failed] {
        let mut t = merging();
        for at in [0, TIMEOUT as i64, TIMEOUT as i64 * 10] {
            t = look(&t, vec![check(state)], at);
            assert_eq!(t.review.note, None, "{state:?} at {at}");
        }
    }
}

#[test]
fn a_check_that_shows_up_late_withdraws_the_question() {
    let t = look(&asked(), vec![check(CheckState::Pending)], 700);
    assert_eq!(t.review.note, None);
    assert_eq!(t.pr.as_ref().and_then(|p| p.no_checks_since), None);
}

#[test]
fn merge_now_merges_without_checks_and_stops_asking() {
    let tr = apply(&asked(), &TicketEvent::MergeUnchecked, NOW).expect("merge now");
    assert_eq!(tr.effects, vec![Effect::Merge]);
    assert_eq!(tr.ticket.review.note, None);
    assert_eq!(tr.ticket.review.turn, Some(ReviewTurn::Merging));
    assert!(tr.ticket.pr.as_ref().is_some_and(|p| p.merge_unchecked));
    // The next look keeps the answer and does not ask again.
    let t = look(&tr.ticket, Vec::new(), 5000);
    assert_eq!(t.review.note, None);
    assert!(t.pr.as_ref().is_some_and(|p| p.merge_unchecked));
}

#[test]
fn keep_waiting_starts_the_timeout_again() {
    let tr = apply(&asked(), &TicketEvent::KeepWaiting, Timestamp(700_000)).expect("keep waiting");
    assert!(tr.effects.is_empty());
    assert_eq!(tr.ticket.review.note, None);
    assert_eq!(tr.ticket.review.turn, Some(ReviewTurn::Merging));
    let t = look(&tr.ticket, Vec::new(), 700 + TIMEOUT as i64 - 1);
    assert_eq!(t.review.note, None);
    let t = look(&t, Vec::new(), 700 + TIMEOUT as i64);
    assert_eq!(t.review.note, Some(ReviewNote::NoChecks));
}

#[test]
fn block_blocks_the_card_with_the_persons_reason() {
    let reason = BlockedReason::Manual {
        text: "no CI here, wait for the owner".to_owned(),
    };
    let tr = apply(&asked(), &TicketEvent::Block { reason }, NOW).expect("block");
    assert_eq!(tr.ticket.status, TicketStatus::Blocked);
    assert_eq!(tr.ticket.review.note, None);
}

#[test]
fn the_three_answers_exist_only_while_the_question_is_open() {
    let names = |t: &Ticket| -> Vec<&'static str> {
        TicketAction::allowed(t)
            .into_iter()
            .map(TicketAction::name)
            .filter(|n| ["merge_now", "keep_waiting", "block"].contains(n))
            .collect()
    };
    assert_eq!(names(&asked()), ["block", "merge_now", "keep_waiting"]);
    assert!(names(&merging()).is_empty());
    let waiting = look(&merging(), Vec::new(), 1);
    assert!(names(&waiting).is_empty());
    for event in [TicketEvent::MergeUnchecked, TicketEvent::KeepWaiting] {
        assert!(apply(&waiting, &event, NOW).is_err(), "{event:?}");
    }
}
