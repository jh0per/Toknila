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
        WaitingReason::Permission { text: "p".into() },
    );
    let mut d = data(vec![t.clone()], 100);
    d.permissions.push(PendingPermission {
        ticket: t.id,
        role: AgentRole::Worker,
        text: "p".into(),
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
