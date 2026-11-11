//! The questions a run leaves besides the card's status: a reviewer's permission dialog, and an
//! agent that waits on its screen.

use super::*;
use crate::AgentRole;
use crate::question::{AnswerKind, PendingPermission, RunFacts, SCREEN_TEXT};

fn data(tickets: Vec<Ticket>, now: i64) -> InboxData {
    InboxData {
        now: Timestamp(now),
        tickets,
        ..InboxData::default()
    }
}

fn run(t: &Ticket, role: AgentRole, tail: &str) -> RunFacts {
    RunFacts {
        ticket: t.id,
        role,
        spawned: Timestamp(1_000),
        first_activity: None,
        startup_secs: 20,
        tail: tail.as_bytes().to_vec(),
        answered: None,
    }
}

#[test]
fn a_workers_question_says_whose_it_is_and_what_answers_it_takes() {
    let q = waiting(
        ticket(1, TicketStatus::Running, 5),
        WaitingReason::Question {
            text: "why?".into(),
        },
    );
    let p = waiting(
        ticket(2, TicketStatus::Running, 6),
        WaitingReason::Permission {
            text: "Bash: ls".into(),
            detail: None,
        },
    );
    let got = compute(&data(vec![q, p], 9));
    assert_eq!(got.len(), 2);
    for i in &got {
        assert_eq!(i.role, Some(AgentRole::Worker));
    }
    assert_eq!(got[0].answers, [AnswerKind::Allow, AnswerKind::Deny]);
    assert!(
        got[1].answers.is_empty(),
        "a question is answered by a comment"
    );
}

#[test]
fn a_reviewers_permission_dialog_is_listed_with_allow_and_deny() {
    let mut t = ticket(1, TicketStatus::Review, 5);
    t.review.turn = Some(ReviewTurn::Agent { decides: true });
    let mut d = data(vec![t.clone()], 100);
    d.permissions.push(PendingPermission {
        ticket: t.id,
        role: AgentRole::Reviewer,
        text: "Claude needs your permission: Bash: git diff".into(),
        detail: None,
        since: Timestamp(50),
    });
    let got = compute(&d);
    assert_eq!(got.len(), 1);
    let i = &got[0];
    assert_eq!(i.kind, InboxKind::Question);
    assert_eq!(i.role, Some(AgentRole::Reviewer));
    assert_eq!(i.text, "Claude needs your permission: Bash: git diff");
    assert_eq!(i.since, Timestamp(50));
    assert_eq!(i.answers, [AnswerKind::Allow, AnswerKind::Deny]);
}

#[test]
fn the_workers_permission_is_not_listed_twice_while_its_card_is_waiting() {
    let t = waiting(
        ticket(1, TicketStatus::Running, 5),
        WaitingReason::Permission {
            text: "p".into(),
            detail: None,
        },
    );
    let mut d = data(vec![t.clone()], 100);
    d.permissions.push(PendingPermission {
        ticket: t.id,
        role: AgentRole::Worker,
        text: "p".into(),
        detail: None,
        since: Timestamp(5),
    });
    assert_eq!(compute(&d).len(), 1);
}

#[test]
fn a_run_with_no_sign_of_life_after_its_startup_time_waits_on_its_screen() {
    let t = ticket(1, TicketStatus::Running, 5);
    let mut d = data(vec![t.clone()], 21_000);
    d.runs.push(run(
        &t,
        AgentRole::Worker,
        "\x1b[1mDo you trust this folder?\x1b[0m\r\n> 1. Yes\r\n  2. No\r\n",
    ));
    let got = compute(&d);
    assert_eq!(got.len(), 1);
    let i = &got[0];
    assert_eq!(
        (i.kind, i.text.as_str()),
        (InboxKind::Question, SCREEN_TEXT)
    );
    assert_eq!(i.role, Some(AgentRole::Worker));
    assert_eq!(
        i.lines,
        ["Do you trust this folder?", "> 1. Yes", "  2. No"]
    );
    assert_eq!(
        i.answers,
        [
            AnswerKind::Enter,
            AnswerKind::One,
            AnswerKind::Two,
            AnswerKind::Up,
            AnswerKind::Down,
            AnswerKind::Escape,
            AnswerKind::Text
        ]
    );
    assert_eq!(i.since, Timestamp(21_000), "since the startup time ran out");
}

#[test]
fn the_screen_item_waits_for_the_startup_time_and_goes_with_the_first_activity() {
    let t = ticket(1, TicketStatus::Running, 5);
    let mut r = run(&t, AgentRole::Worker, "x\n");
    let mut early = data(vec![t.clone()], 20_999);
    early.runs.push(r.clone());
    assert!(compute(&early).is_empty());
    let mut late = data(vec![t.clone()], 60_000);
    late.runs.push(r.clone());
    assert_eq!(compute(&late).len(), 1);
    r.first_activity = Some(Timestamp(30_000));
    let mut active = data(vec![t], 60_000);
    active.runs.push(r);
    assert!(compute(&active).is_empty());
}

#[test]
fn a_reviewer_run_that_waits_on_its_screen_is_listed_as_the_reviewers() {
    let mut t = ticket(1, TicketStatus::Review, 5);
    t.review.turn = Some(ReviewTurn::Agent { decides: true });
    let mut d = data(vec![t.clone()], 30_000);
    d.runs.push(run(&t, AgentRole::Reviewer, "trust?\n"));
    let got = compute(&d);
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].role, Some(AgentRole::Reviewer));
}

#[test]
fn no_screen_item_for_a_card_that_is_not_working_or_already_asks() {
    let waiting_card = waiting(
        ticket(1, TicketStatus::Running, 5),
        WaitingReason::Question { text: "q".into() },
    );
    let blocked_card = blocked(
        ticket(2, TicketStatus::Running, 5),
        BlockedReason::Manual { text: "b".into() },
    );
    let running = ticket(3, TicketStatus::Running, 5);
    let mut d = data(
        vec![waiting_card.clone(), blocked_card.clone(), running.clone()],
        60_000,
    );
    d.runs.push(run(&waiting_card, AgentRole::Worker, "x\n"));
    d.runs.push(run(&blocked_card, AgentRole::Worker, "x\n"));
    d.runs.push(run(&running, AgentRole::Worker, "x\n"));
    d.permissions.push(PendingPermission {
        ticket: running.id,
        role: AgentRole::Worker,
        text: "p".into(),
        detail: None,
        since: Timestamp(1),
    });
    let shown: Vec<_> = compute(&d)
        .iter()
        .map(|i| (i.ticket.to_string(), i.kind, i.text.clone()))
        .collect();
    assert_eq!(
        shown,
        [
            ("DEMO-1".to_owned(), InboxKind::Question, "q".to_owned()),
            (
                "DEMO-2".to_owned(),
                InboxKind::Blocked,
                "manual: b".to_owned()
            ),
            ("DEMO-3".to_owned(), InboxKind::Question, "p".to_owned()),
        ],
        "no screen item beside a question, a blocked card or a dialog"
    );
}

#[test]
fn a_reviewer_without_a_verdict_is_a_review_item_with_its_screen_and_run_again() {
    let mut t = ticket(1, TicketStatus::Review, 5);
    t.review.turn = Some(crate::ReviewTurn::Person);
    t.review.note = Some(crate::ReviewNote::NoVerdict {
        lines: vec![
            "I could not run cargo".into(),
            "what I need from you".into(),
        ],
    });
    let got = compute(&data(vec![t], 9));
    assert_eq!(got.len(), 1);
    let i = &got[0];
    assert_eq!(
        (i.kind, i.text.as_str()),
        (InboxKind::Review, NO_VERDICT_TEXT)
    );
    assert_eq!(i.lines, ["I could not run cargo", "what I need from you"]);
    assert!(i.allowed_actions.contains(&TicketAction::RunReviewer));
    assert!(i.allowed_actions.contains(&TicketAction::Accept));
    assert!(i.allowed_actions.contains(&TicketAction::Return));
}

#[test]
fn a_check_that_cannot_run_is_a_review_item_that_says_what_to_fix() {
    let mut t = ticket(1, TicketStatus::Review, 5);
    t.review.turn = Some(crate::ReviewTurn::Person);
    t.review.note = Some(crate::ReviewNote::CheckCannotRun {
        command: "cargo test".into(),
    });
    let got = compute(&data(vec![t], 9));
    assert_eq!(got.len(), 1);
    let i = &got[0];
    assert_eq!(i.kind, InboxKind::Review);
    assert!(i.text.contains("`cargo test` could not run"), "{}", i.text);
    assert!(
        i.text.contains("Add ~/.cargo/bin to the PATH"),
        "{}",
        i.text
    );
    assert!(!i.allowed_actions.contains(&TicketAction::RunReviewer));
    assert!(i.allowed_actions.contains(&TicketAction::Accept));
    assert!(i.allowed_actions.contains(&TicketAction::Return));
}

#[test]
fn a_screen_answered_by_keys_stays_quiet_while_it_is_unchanged() {
    let t = ticket(1, TicketStatus::Running, 5);
    let mut r = run(&t, AgentRole::Worker, "Trust this folder?\n> Yes\n");
    r.answered = Some(crate::question::ScreenAnswer {
        at: Timestamp(30_000),
        screen: crate::question::screen_hash(&r.tail),
    });
    let mut d = data(vec![t], 60_000);
    d.runs = vec![r];
    assert!(compute(&d).is_empty(), "30 s after the answer, same screen");
    d.now = Timestamp(90_000);
    assert_eq!(compute(&d).len(), 1, "the grace has passed");
}

#[test]
fn an_unfounded_request_for_changes_reaches_the_person_with_the_reviewers_reading() {
    use crate::machine::{TicketEvent, apply};
    let mut t = ticket(1, TicketStatus::Review, 5);
    t.acceptance = vec!["it builds".to_owned()];
    t.review.turn = Some(crate::ReviewTurn::Agent { decides: false });
    let event = TicketEvent::ReviewVerdict {
        verdict: crate::Verdict::RequestChanges,
        comment: "Bash was refused".to_owned(),
        items: vec![crate::ItemVerdict {
            item: 1,
            outcome: crate::ItemOutcome::Fail,
            basis: crate::ItemBasis::NotRun,
            note: "cargo test refused".to_owned(),
        }],
        max_rounds: 3,
    };
    let t = apply(&t, &event, crate::Timestamp(9))
        .expect("heard")
        .ticket;
    let got = compute(&data(vec![t], 9));
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].kind, InboxKind::Review);
    assert_eq!(
        got[0].lines,
        [
            "NOT_RUN 1. it builds - cargo test refused [NOT_RUN]",
            "Bash was refused"
        ]
    );
    assert!(got[0].allowed_actions.contains(&TicketAction::RunReviewer));
}
