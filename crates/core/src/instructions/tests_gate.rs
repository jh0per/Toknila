//! The reviewer is given the daemon's own result of the gate.

use super::*;
use crate::{AgentRole, CheckOutcome, Timestamp};

fn brief_with<'a>(check: Option<&'a CheckOutcome>) -> RunBrief<'a> {
    RunBrief {
        key: "DEMO-3",
        title: "t",
        prompt: "p",
        acceptance: &[],
        check_command: Some("cargo xtask verify"),
        setup_command: None,
        earlier: &[],
        restarted: false,
        waiting: &[],
        returned: None,
        design_first: false,
        designs: &[],
        has_designs: false,
        discussion: &[],
        epic: None,
        check,
        threads: &[],
        hand_in: None,
    }
}

fn outcome(passed: bool) -> CheckOutcome {
    CheckOutcome {
        command: "cargo xtask verify".to_owned(),
        passed,
        output: "`cargo xtask verify` exited with code 0\ntest result: ok. 412 passed".to_owned(),
        at: Timestamp(1_700_000_000_000),
    }
}

#[test]
fn the_reviewer_kickoff_carries_the_command_the_verdict_and_the_output() {
    let pass = outcome(true);
    let text = kickoff(AgentRole::Reviewer, &brief_with(Some(&pass)));
    assert!(text.contains("`cargo xtask verify`"), "{text}");
    assert!(
        text.contains("with PASS") && !text.contains("with FAIL"),
        "{text}"
    );
    assert!(text.contains("test result: ok. 412 passed"), "{text}");
    assert!(text.contains("2023-11-14 22:13:20 UTC"), "{text}");
    assert!(text.contains("do not run the whole gate again"), "{text}");
    let fail = outcome(false);
    let text = kickoff(AgentRole::Reviewer, &brief_with(Some(&fail)));
    assert!(text.contains("with FAIL"), "{text}");
}

#[test]
fn without_a_result_the_reviewer_is_told_toknila_did_not_run_the_gate() {
    let text = kickoff(AgentRole::Reviewer, &brief_with(None));
    assert!(text.contains("Toknila did not run the gate"), "{text}");
    assert!(text.contains("`NOT_RUN`"), "{text}");
    assert!(!text.contains("with PASS"), "{text}");
}

#[test]
fn the_review_instructions_carry_the_result_too() {
    let pass = outcome(true);
    let text = generate_review(&brief_with(Some(&pass)));
    assert!(text.contains("## The gate's result"), "{text}");
    assert!(text.contains("test result: ok. 412 passed"), "{text}");
    assert!(!text.contains("## Checks"), "{text}");
    let text = generate_review(&brief_with(None));
    assert!(text.contains("Toknila did not run the gate"), "{text}");
}

#[test]
fn utc_text_is_the_calendar_date_of_the_instant() {
    assert_eq!(Timestamp(0).utc_text(), "1970-01-01 00:00:00 UTC");
    assert_eq!(
        Timestamp(951_782_400_000 + 86_399_000).utc_text(),
        "2000-02-29 23:59:59 UTC"
    );
}
