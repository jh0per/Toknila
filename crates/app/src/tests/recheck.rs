//! Accept checks again when main moved since the last check, and the extra checks of a project
//! run for the files a card changed. Through the use cases, with a fake merger that says what
//! main is at, a fake checker and a fake shipyard that lists the changed files.

use toknila_core::machine::TicketEvent;
use toknila_core::{ExtraCheck, ReviewMode, ReviewTurn, TicketStatus};

use super::Mem;
use super::checks::{answer, fails, running};
use super::runs::{a, finish, settle};
use crate::{Seen, tickets};

fn main_at(mem: &Mem, base: &str, commits: Option<u32>) {
    *mem.merger.seen_answer.borrow_mut() = Some(Seen {
        base: base.to_owned(),
        head: format!("head-of-{base}"),
        commits,
    });
}

/// A card in Review for a person whose hand-in check passed with main at `m1`.
fn checked_at_m1() -> Mem {
    let mut mem = running(ReviewMode::Human, 3);
    main_at(&mem, "m1", None);
    finish(&mut mem, "done".to_owned());
    settle(&mut mem);
    let t = a(&mem);
    assert_eq!(t.review.turn, Some(ReviewTurn::Person));
    let kept = t.review.last_check.expect("the check is recorded");
    assert_eq!(
        (kept.base.as_deref(), kept.head.as_deref()),
        (Some("m1"), Some("head-of-m1"))
    );
    mem
}

fn accept(mem: &mut Mem) {
    let t = a(mem);
    tickets::act(mem, &t, TicketEvent::Accept { comment: None }).expect("accept");
}

#[test]
fn accept_after_main_moved_checks_once_more_and_then_merges_by_itself() {
    let mut mem = checked_at_m1();
    main_at(&mem, "m2", Some(3));
    accept(&mut mem);
    settle(&mut mem);
    let t = a(&mem);
    assert_eq!(
        t.status,
        TicketStatus::Merged,
        "no second Accept was needed"
    );
    assert_eq!(
        mem.checker.asked.borrow().len(),
        2,
        "the hand-in and the re-check"
    );
    assert_eq!(
        mem.merger.asked.borrow().len(),
        1,
        "merged once, after the pass"
    );
    let comments: Vec<_> = t.review.rounds.iter().map(|r| r.comment.as_str()).collect();
    assert!(
        comments.contains(&"main moved since the check: 3 commits"),
        "{comments:?}"
    );
}

#[test]
fn accept_with_main_where_the_check_saw_it_merges_at_once() {
    let mut mem = checked_at_m1();
    accept(&mut mem);
    settle(&mut mem);
    assert_eq!(a(&mem).status, TicketStatus::Merged);
    assert_eq!(
        mem.checker.asked.borrow().len(),
        1,
        "the check is not run again"
    );
}

#[test]
fn a_failing_check_again_is_a_round_for_the_agent() {
    let mut mem = checked_at_m1();
    main_at(&mem, "m2", Some(1));
    answer(&mem, [fails("error: broke on the new main")]);
    accept(&mut mem);
    settle(&mut mem);
    let t = a(&mem);
    assert_eq!(t.status, TicketStatus::Running, "back to the agent");
    assert!(mem.merger.asked.borrow().is_empty(), "nothing merged");
    assert!(t.review.accepted, "the Accept is remembered");
    let last = t.review.rounds.last().expect("round");
    assert!(
        last.comment.contains("broke on the new main"),
        "{}",
        last.comment
    );
    assert!(
        t.review
            .rounds
            .iter()
            .any(|r| r.comment == "main moved since the check: 1 commit"),
        "the round names why it ran again"
    );
}

fn with_extras(files: &[&str]) -> Mem {
    let mut mem = running(ReviewMode::Human, 3);
    mem.project.extra_checks = vec![
        ExtraCheck::parse("apps/web/**=web-gate one").expect("extra"),
        ExtraCheck::parse("docs/**=docs-gate").expect("extra"),
        ExtraCheck::parse("apps/**=apps-gate").expect("extra"),
    ];
    *mem.yard.changed_files.borrow_mut() = files.iter().map(|f| (*f).to_owned()).collect();
    main_at(&mem, "m1", None);
    mem
}

#[test]
fn an_extra_check_runs_only_for_matching_files_in_order_and_has_its_line_in_the_round() {
    let mut mem = with_extras(&["apps/web/src/a.ts", "crates/x.rs"]);
    finish(&mut mem, "done".to_owned());
    settle(&mut mem);
    let ran: Vec<String> = mem
        .checker
        .asked
        .borrow()
        .iter()
        .map(|s| s.command.clone())
        .collect();
    assert_eq!(
        ran,
        ["cargo xtask verify", "web-gate one", "apps-gate"],
        "the command first, then the matching extras in the project's order; docs did not match"
    );
    let t = a(&mem);
    assert_eq!(t.review.turn, Some(ReviewTurn::Person));
    let round = t.review.rounds.last().expect("round");
    assert!(
        round.comment.starts_with("The check passed."),
        "{}",
        round.comment
    );
    assert!(
        round
            .comment
            .contains("extra check apps/web/**: PASS web-gate one, 0 s"),
        "{}",
        round.comment
    );
    assert!(
        round
            .comment
            .contains("extra check apps/**: PASS apps-gate"),
        "{}",
        round.comment
    );
    assert!(!round.comment.contains("docs-gate"));
}

#[test]
fn a_failing_extra_check_fails_the_round_with_its_output_and_stops_the_rest() {
    let mut mem = with_extras(&["apps/web/src/a.ts"]);
    // The command passes; the first extra check that matches fails.
    answer(
        &mem,
        [
            Ok(crate::checks::CheckResult {
                success: true,
                code: Some(0),
                timed_out: false,
                output: "ok".to_owned(),
            }),
            fails("web spec failed: login.mjs"),
        ],
    );
    finish(&mut mem, "done".to_owned());
    settle(&mut mem);
    let ran: Vec<String> = mem
        .checker
        .asked
        .borrow()
        .iter()
        .map(|s| s.command.clone())
        .collect();
    assert_eq!(
        ran,
        ["cargo xtask verify", "web-gate one"],
        "apps-gate did not run after the failure"
    );
    let t = a(&mem);
    assert_eq!(t.status, TicketStatus::Running, "back to the agent");
    let round = t.review.rounds.last().expect("round");
    assert!(
        round
            .comment
            .contains("extra check apps/web/**: FAIL web-gate one"),
        "{}",
        round.comment
    );
    assert!(
        round.comment.contains("web spec failed: login.mjs"),
        "{}",
        round.comment
    );
}

#[test]
fn no_extra_check_runs_when_no_changed_file_matches() {
    let mut mem = with_extras(&["crates/x.rs"]);
    finish(&mut mem, "done".to_owned());
    settle(&mut mem);
    assert_eq!(
        mem.checker.asked.borrow().len(),
        1,
        "only the check command"
    );
    let round = a(&mem).review.rounds.last().cloned().expect("round");
    assert_eq!(round.comment, "The check passed.");
}
