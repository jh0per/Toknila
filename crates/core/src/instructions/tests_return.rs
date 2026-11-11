use super::*;
use crate::{Actor, CommentBody, CommentId, TicketId, Timestamp};

fn instruction(gist: &str, what: Option<&str>) -> Comment {
    Comment {
        id: CommentId::new(),
        ticket_id: TicketId::new(),
        author: Actor::Human,
        kind: CommentKind::Instruction,
        body: CommentBody {
            gist: gist.to_owned(),
            what: what.map(str::to_owned),
            ..CommentBody::default()
        },
        created_at: Timestamp(1),
        delivered_at: None,
    }
}

fn brief(acceptance: &[String]) -> RunBrief<'_> {
    RunBrief {
        key: "DEMO-3",
        title: "Return",
        prompt: "Work.\n",
        acceptance,
        check_command: None,
        earlier: &[],
        waiting: &[],
        returned: None,
        design_first: false,
        designs: &[],
        has_designs: false,
    }
}

fn returned_round(by: Actor, text: &str) -> Returned {
    let mut review = crate::Review {
        handed_in: Some("f8afe06aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned()),
        ..crate::Review::default()
    };
    review.record(by, crate::Verdict::RequestChanges, text, Timestamp(5));
    Returned::of(&review).expect("a return")
}

#[test]
fn a_pending_instruction_is_in_the_file_and_the_kickoff_says_it_comes_first() {
    let acceptance = vec!["Pending is read first".to_owned()];
    let mut done = instruction("already delivered one", None);
    done.delivered_at = Some(Timestamp(3));
    let pending = instruction("use the Edit tool", Some("do it"));
    let w = waiting(&[done, pending.clone()], None);
    assert_eq!(w.ids, vec![pending.id]);
    assert_eq!(w.texts.len(), 1);
    let with = RunBrief {
        waiting: &w.texts,
        ..brief(&acceptance)
    };
    let text = generate(&with);
    assert!(text.contains("## Instructions waiting for you"), "{text}");
    assert!(
        text.contains("> # [instruction] use the Edit tool"),
        "{text}"
    );
    assert!(!text.contains("already delivered one"));
    let first = kickoff(crate::AgentRole::Worker, &with);
    assert!(
        first.contains("Instructions are waiting for you"),
        "{first}"
    );
    assert!(!generate(&brief(&acceptance)).contains("waiting for you"));
    assert!(!kickoff(crate::AgentRole::Worker, &brief(&acceptance)).contains("waiting"));
}

#[test]
fn a_run_after_a_return_starts_with_the_round_who_the_commit_and_the_text() {
    let acceptance = vec!["The return is read first".to_owned()];
    let returned = returned_round(Actor::Human, "Rename the flag\nand update the docs");
    let with = RunBrief {
        returned: Some(&returned),
        ..brief(&acceptance)
    };
    let first = kickoff(crate::AgentRole::Worker, &with);
    for want in [
        "round 1",
        "by the person",
        "hand-in f8afe06",
        "Rename the flag and update the docs",
        "naming the new commit",
        "same commit is refused",
    ] {
        assert!(first.contains(want), "{want}: {first}");
    }
    let text = generate(&with);
    assert!(text.contains("## This card was returned"), "{text}");
    assert!(
        text.contains("> Rename the flag\n> and update the docs"),
        "{text}"
    );
    assert!(text.contains("commit f8afe06"));
    assert_eq!(
        returned_round(Actor::Agent, "x").by,
        Some(Actor::Agent),
        "a reviewer's request is a Return too"
    );
    assert!(!generate(&brief(&acceptance)).contains("This card was returned"));
}

#[test]
fn a_comment_that_only_repeats_the_return_is_not_shown_twice_but_is_taken() {
    let returned = returned_round(Actor::Human, "Rename the flag");
    let repeat = instruction("Returned", Some("Rename the flag"));
    let other = instruction("Also mind the docs", None);
    let w = waiting(&[repeat, other], Some(&returned));
    assert_eq!(w.ids.len(), 2);
    assert_eq!(w.texts.len(), 1);
    assert!(w.texts[0].contains("mind the docs"));
}

#[test]
fn a_card_that_was_approved_last_has_no_open_return() {
    let mut review = crate::Review::default();
    review.record(
        Actor::Human,
        crate::Verdict::RequestChanges,
        "x",
        Timestamp(1),
    );
    review.record(Actor::Human, crate::Verdict::Approve, "", Timestamp(2));
    assert!(Returned::of(&review).is_none());
}
