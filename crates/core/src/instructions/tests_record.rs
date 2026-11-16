use super::*;
use crate::{ThreadKind, ThreadState, Timestamp};

fn two_threads() -> Vec<BriefThread> {
    vec![
        BriefThread {
            number: 1,
            kind: ThreadKind::Design,
            title: "The Review block".to_owned(),
            state: ThreadState::Approved,
            approved_version: Some(2),
            approved_at: Some(Timestamp(1_700_000_000_000)),
        },
        BriefThread {
            number: 2,
            kind: ThreadKind::Flow,
            title: "The verdict flow".to_owned(),
            state: ThreadState::Open,
            approved_version: None,
            approved_at: None,
        },
    ]
}

const HAND_IN: &str = "**What changed:** PERSON is a verdict word\nthe reviewer sees the record\n**Checks:**\n- PASS cargo xtask verify: test result: ok. 412 passed\n**Left:** nothing";

#[test]
fn the_reviewer_reads_the_threads_and_the_whole_hand_in_in_its_instructions() {
    let threads = two_threads();
    let brief = RunBrief {
        key: "DEMO-3",
        title: "t",
        prompt: "p",
        threads: &threads,
        hand_in: Some(HAND_IN),
        ..RunBrief::default()
    };
    let text = generate_review(&brief);
    assert!(
        text.contains(
            "- thread 1 (design): The Review block, approved version 2 by a person at 2023-11-14 22:13:20 UTC\n"
        ),
        "{text}"
    );
    assert!(
        text.contains("- thread 2 (flow): The verdict flow, open, not approved\n"),
        "{text}"
    );
    for line in HAND_IN.lines() {
        assert!(text.contains(&format!("> {line}\n")), "{line}: {text}");
    }
    // The worker's instructions carry neither.
    let worker = generate(&brief);
    assert!(!worker.contains("The card's threads") && !worker.contains("## The hand-in"));
    // Nothing to show says so, and an empty hand-in has no section.
    let none = generate_review(&RunBrief {
        key: "DEMO-3",
        ..RunBrief::default()
    });
    assert!(none.contains("The story has no design threads."));
    assert!(none.contains("no hand-in was recorded"));
}

fn position(text: &str, needle: &str) -> usize {
    text.find(needle)
        .unwrap_or_else(|| panic!("{needle}: {text}"))
}

#[test]
fn the_hand_in_section_follows_the_gate_and_the_kickoff_points_to_it() {
    let brief = RunBrief {
        key: "DEMO-3",
        hand_in: Some(HAND_IN),
        ..RunBrief::default()
    };
    let text = generate_review(&brief);
    assert!(position(&text, "## The hand-in") > position(&text, "## The gate's result"));
    assert!(position(&text, "## The hand-in") < position(&text, "## Work alone"));
    let kick = kickoff(crate::AgentRole::Reviewer, &brief);
    assert!(kick.contains("(in The hand-in)"), "{kick}");
}

#[test]
fn without_a_recorded_hand_in_the_section_says_so() {
    let text = generate_review(&RunBrief {
        key: "DEMO-3",
        ..RunBrief::default()
    });
    assert!(text.contains("## The hand-in"), "{text}");
    assert!(text.contains("no hand-in was recorded"), "{text}");
}

#[test]
fn after_a_return_the_hand_in_section_carries_since_the_return() {
    let returned = super::Returned {
        since: Some(crate::SinceReturn::new(
            1,
            "f8afe06aaaaaaaa",
            "b0b0b0bbbbbbbbb",
            vec!["rename flag".to_owned()],
            vec!["flag.txt".to_owned()],
        )),
        ..super::Returned::default()
    };
    let text = generate_review(&RunBrief {
        key: "DEMO-3",
        hand_in: Some(HAND_IN),
        returned: Some(&returned),
        ..RunBrief::default()
    });
    let hand_in = position(&text, "## The hand-in");
    let since = position(&text, "Since the Return");
    assert!(
        since > hand_in && since < position(&text, "## Work alone"),
        "{text}"
    );
    assert!(text.contains("flag.txt"), "{text}");
}

#[test]
fn the_reviewer_is_told_the_command_forms_that_pass_in_dont_ask_mode() {
    let text = generate_review(&RunBrief::default());
    for want in [
        "git log --pretty=fuller main..HEAD",
        "git show --stat <sha>",
        "grep -rn <word> <folder>",
        "no backslashes, no quotes, no `%` format strings, no globs, no pipes or redirects",
        "rewrite it once in a plain form",
    ] {
        assert!(text.contains(want), "{want}: {text}");
    }
}

#[test]
fn the_review_protocol_names_the_person_verdict() {
    let text = generate_review(&RunBrief::default());
    assert!(
        text.contains("`outcome: person` (shown as PERSON)"),
        "{text}"
    );
    assert!(text.contains("Never ask for files the story did not ask for"));
}
