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
    assert!(none.contains("The card has no design threads."));
    assert!(!none.contains("## The hand-in"));
}

#[test]
fn the_review_protocol_names_the_person_verdict() {
    let text = generate_review(&RunBrief::default());
    assert!(
        text.contains("`outcome: person` (shown as PERSON)"),
        "{text}"
    );
    assert!(text.contains("Never ask for files the card did not ask for"));
}
