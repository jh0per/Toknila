//! Accept checks again when main moved since the last check: the check records the main commit
//! and the ship's HEAD it saw, `MainMoved` sends the accepted card back through the check turn,
//! and a pass of that check merges without a second Accept.

use super::*;
use crate::{CheckPhase, Recheck, ReviewMode, ReviewTurn, Verdict};

fn step(t: &Ticket, event: &TicketEvent) -> Transition {
    let tr = apply(t, event, NOW).unwrap_or_else(|e| panic!("{event:?}: {e}"));
    check_invariants(&tr.ticket).unwrap_or_else(|e| panic!("{event:?}: {e}"));
    tr
}

fn checking() -> Ticket {
    let mut t = ticket_in(TicketStatus::Review);
    t.review.turn = Some(ReviewTurn::Check);
    t.review.check = Some(CheckPhase::Running {
        command: "cargo xtask verify".to_owned(),
        since: T0,
    });
    t
}

fn done(passed: bool, base: Option<&str>, head: Option<&str>) -> TicketEvent {
    TicketEvent::CheckDone {
        passed,
        output: "out".to_owned(),
        base: base.map(str::to_owned),
        head: head.map(str::to_owned),
        lines: vec!["extra check apps/web/**: PASS cargo xtask web, 4 min".to_owned()],
        flaky: None,
        mode: ReviewMode::Both,
        max_rounds: 3,
    }
}

#[test]
fn a_flaky_pass_says_so_in_its_round_and_is_recorded() {
    let TicketEvent::CheckDone {
        passed,
        output,
        base,
        head,
        lines,
        mode,
        max_rounds,
        ..
    } = done(true, None, None)
    else {
        unreachable!()
    };
    let event = TicketEvent::CheckDone {
        passed,
        output,
        base,
        head,
        lines,
        flaky: Some("test einval ... FAILED".to_owned()),
        mode,
        max_rounds,
    };
    let tr = step(&checking(), &event);
    let kept = tr.ticket.review.last_check.expect("kept");
    assert!(kept.passed && kept.flaky);
    assert_eq!(kept.flaky_output, "test einval ... FAILED");
    let round = tr.ticket.review.rounds.last().expect("round");
    assert_eq!(
        round.comment,
        "The check passed on the second run; the first failed:\ntest einval ... FAILED\nextra check apps/web/**: PASS cargo xtask web, 4 min"
    );
    assert_eq!(round.verdict, Verdict::Approve);
}

/// A card a person accepted whose merge is running, its last check having seen main at `base`.
fn merging(base: Option<&str>) -> Ticket {
    let checked = step(&checking(), &done(true, base, Some("h1"))).ticket;
    let mut t = checked;
    t.review.turn = Some(ReviewTurn::Person);
    step(&t, &TicketEvent::Accept { comment: None }).ticket
}

#[test]
fn check_done_records_the_main_commit_and_the_ship_head() {
    let tr = step(&checking(), &done(true, Some("aaa111"), Some("bbb222")));
    let kept = tr.ticket.review.last_check.expect("kept");
    assert_eq!(kept.base.as_deref(), Some("aaa111"));
    assert_eq!(kept.head.as_deref(), Some("bbb222"));
    // The passing round carries the extra checks' lines under the verdict.
    let round = tr.ticket.review.rounds.last().expect("round");
    assert_eq!(
        round.comment,
        "The check passed.\nextra check apps/web/**: PASS cargo xtask web, 4 min"
    );
    let failed = step(&checking(), &done(false, Some("aaa111"), None));
    let kept = failed.ticket.review.last_check.expect("kept");
    assert_eq!((kept.base.as_deref(), kept.head), (Some("aaa111"), None));
}

#[test]
fn accept_with_main_moved_goes_back_through_the_check_and_merges_by_itself() {
    let t = merging(Some("aaa"));
    assert_eq!(t.review.turn, Some(ReviewTurn::Merging));
    // The host found main at another commit than the last check saw.
    assert!(Recheck::of(t.review.last_check.as_ref()).due("bbb"));
    let moved = step(
        &t,
        &TicketEvent::MainMoved {
            text: "main moved since the check: 3 commits".to_owned(),
        },
    );
    assert_eq!(moved.ticket.status, TicketStatus::Review);
    assert_eq!(moved.ticket.review.turn, Some(ReviewTurn::Check));
    assert!(moved.ticket.review.recheck);
    assert_eq!(moved.effects, vec![Effect::RunCheck]);
    let round = moved.ticket.review.rounds.last().expect("a round");
    assert_eq!(round.comment, "main moved since the check: 3 commits");
    // A pass merges at once, whatever the review mode: nobody is asked again.
    let passed = step(&moved.ticket, &done(true, Some("bbb"), Some("h2")));
    assert_eq!(passed.ticket.review.turn, Some(ReviewTurn::Merging));
    assert!(passed.effects.contains(&Effect::Merge));
    assert!(!passed.effects.contains(&Effect::StartReviewer));
    assert!(!passed.ticket.review.recheck && !passed.ticket.review.accepted);
    let kept = passed.ticket.review.last_check.expect("kept");
    assert_eq!(kept.base.as_deref(), Some("bbb"));
}

#[test]
fn a_failing_check_again_is_a_round_for_the_agent_and_keeps_the_accept() {
    let t = merging(Some("aaa"));
    let moved = step(
        &t,
        &TicketEvent::MainMoved {
            text: "main moved since the check: 1 commit".to_owned(),
        },
    );
    let failed = step(&moved.ticket, &done(false, Some("bbb"), Some("h2")));
    assert_eq!(failed.ticket.status, TicketStatus::Running);
    assert!(
        failed
            .effects
            .iter()
            .any(|e| matches!(e, Effect::RecordCheckOutput { .. }))
    );
    let last = failed.ticket.review.rounds.last().expect("round");
    assert_eq!(
        (last.reviewer, last.verdict),
        (crate::Actor::System, Verdict::RequestChanges)
    );
    // The Accept is remembered: the next hand-in that passes merges without a second one.
    assert!(failed.ticket.review.accepted && !failed.ticket.review.recheck);
}

#[test]
fn the_merge_goes_on_when_the_recorded_main_is_the_one_it_sees() {
    let t = merging(Some("aaa"));
    assert!(!Recheck::of(t.review.last_check.as_ref()).due("aaa"));
    let done = step(&t, &TicketEvent::MergeDone);
    assert_eq!(done.ticket.status, TicketStatus::Merged);
    // A check that recorded no main is always due.
    assert!(Recheck::of(merging(None).review.last_check.as_ref()).due("aaa"));
}

#[test]
fn main_moved_is_only_for_a_merge_that_is_running() {
    let mut t = ticket_in(TicketStatus::Review);
    t.review.turn = Some(ReviewTurn::Person);
    let event = TicketEvent::MainMoved {
        text: "x".to_owned(),
    };
    assert!(apply(&t, &event, NOW).is_err());
    assert!(apply(&ticket_in(TicketStatus::Running), &event, NOW).is_err());
}
