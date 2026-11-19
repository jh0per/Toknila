use super::tests::{assert_pinned, pinned_brief};
use super::*;
use crate::{Actor, CommentBody, CommentId, TicketId, Timestamp};

fn comment(author: Actor, kind: CommentKind, at: i64, gist: &str, what: Option<&str>) -> Comment {
    Comment {
        id: CommentId::new(),
        ticket_id: TicketId::new(),
        author,
        kind,
        body: CommentBody {
            gist: gist.to_owned(),
            what: what.map(str::to_owned),
            ..CommentBody::default()
        },
        created_at: Timestamp(at),
        delivered_at: None,
    }
}

const DAY: i64 = 86_400_000;
const T0: i64 = 1_700_000_000_000;

fn person(at: i64, text: &str) -> Comment {
    comment(Actor::Human, CommentKind::Instruction, at, text, None)
}

#[test]
fn an_agents_own_report_is_never_the_persons_words() {
    let comments = vec![
        comment(
            Actor::Agent,
            CommentKind::Report,
            T0,
            "I decided to cut the hold",
            None,
        ),
        comment(
            Actor::Agent,
            CommentKind::Decision,
            T0 + 1,
            "Decision: drop it",
            None,
        ),
        comment(
            Actor::System,
            CommentKind::Instruction,
            T0 + 2,
            "Returned by the reviewer",
            None,
        ),
        comment(
            Actor::Agent,
            CommentKind::Instruction,
            T0 + 3,
            "agent note to self",
            None,
        ),
        person(T0 + 4, "Keep the hold."),
    ];
    let told = BriefTold::of(&comments);
    assert_eq!(told.len(), 1, "{told:?}");
    assert_eq!(told[0].text, "Keep the hold.");
    let brief = RunBrief {
        told: &told,
        ..RunBrief::default()
    };
    let text = told::section(&brief);
    assert!(!text.contains("I decided to cut"), "{text}");
    assert!(!text.contains("drop it"), "{text}");
    assert!(!text.contains("Returned by the reviewer"), "{text}");
}

#[test]
fn the_entries_keep_the_order_they_happened_in_and_an_answer_carries_its_question() {
    let comments = vec![
        person(T0 + 3 * DAY, "third"),
        comment(
            Actor::Agent,
            CommentKind::Report,
            T0,
            "Hand in with the hold only?",
            None,
        ),
        person(T0 + DAY, "Hand in with the hold only"),
        person(T0 + 2 * DAY, "second\nwith more lines"),
    ];
    let told = BriefTold::of(&comments);
    let order: Vec<&str> = told.iter().map(|t| t.text.as_str()).collect();
    assert_eq!(
        order,
        [
            "Hand in with the hold only",
            "second\nwith more lines",
            "third"
        ]
    );
    assert_eq!(
        told[0].question.as_deref(),
        Some("Hand in with the hold only?")
    );
    assert_eq!(told[1].question, None);
}

#[test]
fn a_long_record_loses_its_oldest_entries_with_a_count_and_never_the_newest() {
    // Three entries fit under the bound, a fourth does not.
    let big = "x".repeat(TOLD_MAX_CHARS / 4);
    let told: Vec<BriefTold> = (0..5)
        .map(|i| BriefTold {
            at: Timestamp(T0 + i * DAY),
            question: None,
            text: format!("entry-{i} {big}"),
        })
        .collect();
    let text = told::section(&RunBrief {
        told: &told,
        ..RunBrief::default()
    });
    assert!(text.contains("entry-4 "), "the newest is there");
    assert!(text.contains("entry-3 ") && text.contains("entry-2 "));
    assert!(!text.contains("entry-0 ") && !text.contains("entry-1 "));
    assert!(text.contains("2 earlier entries are left out"), "{text}");
    let order = |n: usize| text.find(&format!("entry-{n} ")).expect("present");
    assert!(order(2) < order(3) && order(3) < order(4));
}

#[test]
fn one_entry_over_the_bound_is_still_shown_whole() {
    let huge = "y".repeat(TOLD_MAX_CHARS * 3);
    let told = vec![
        BriefTold {
            at: Timestamp(T0),
            question: None,
            text: "old".to_owned(),
        },
        BriefTold {
            at: Timestamp(T0 + DAY),
            question: None,
            text: huge.clone(),
        },
    ];
    let text = told::section(&RunBrief {
        told: &told,
        ..RunBrief::default()
    });
    assert!(text.contains(&huge));
    assert!(text.contains("1 earlier entry is left out"), "{text}");
}

#[test]
fn nothing_told_says_so() {
    let text = told::section(&RunBrief::default());
    assert!(text.contains("The person told the agent nothing"), "{text}");
}

#[test]
fn the_reviewers_instructions_with_one_instruction_and_one_answer_are_pinned() {
    let comments = vec![
        person(
            T0,
            "Use the licence wording Elastic License 2.0 in the README.",
        ),
        comment(
            Actor::Agent,
            CommentKind::Report,
            T0 + DAY,
            "Hand in with the hold only, or with the whole story?",
            None,
        ),
        person(T0 + 2 * DAY, "Hand in with the hold only"),
    ];
    let told = BriefTold::of(&comments);
    let acceptance = vec!["The review file is pinned by a snapshot test".to_owned()];
    let mut brief = pinned_brief(&acceptance);
    brief.told = &told;
    let text = generate_review(&brief);
    for w in [
        "the person's later word wins",
        "name in the verdict the instruction that changed it",
        "is not the person's decision",
    ] {
        assert!(text.contains(w), "{w}: {text}");
    }
    assert_pinned(&text, "review_told.md");
}
