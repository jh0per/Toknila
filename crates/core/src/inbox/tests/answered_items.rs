//! A question a person answered whose delivery waits for a slot leaves the person's list: it is an
//! informational row that needs nothing from them, until the answer is delivered.

use super::*;
use crate::PausedAgent;

fn paused(mut t: Ticket, resume: bool) -> Ticket {
    t.agent_paused = Some(PausedAgent {
        session: Some("s".to_owned()),
        since: Timestamp(1),
        resume,
        lost: false,
    });
    t
}

fn asking(n: u32, permission: bool) -> Ticket {
    let reason = if permission {
        WaitingReason::Permission {
            text: "Bash: ls".into(),
            detail: None,
        }
    } else {
        WaitingReason::Question {
            text: "why?".into(),
            choices: vec!["yes".into()],
        }
    };
    waiting(ticket(n, TicketStatus::Waiting, 5), reason)
}

fn with_line(tickets: Vec<Ticket>, line: &[(usize, u32)]) -> Vec<InboxItem> {
    let positions = line.iter().map(|(i, n)| (tickets[*i].id, *n)).collect();
    compute(&InboxData {
        tickets,
        positions,
        ..InboxData::default()
    })
}

#[test]
fn an_answered_question_waiting_for_a_slot_is_not_a_person_item_and_says_its_place_in_line() {
    for permission in [false, true] {
        let answered = paused(asking(1, permission), true);
        let got = with_line(vec![answered], &[(0, 2)]);
        assert_eq!(got.len(), 1, "{got:?}");
        let i = &got[0];
        assert_ne!(i.kind, InboxKind::Question);
        assert_eq!(i.kind, InboxKind::Answered);
        assert!(!i.needs_human);
        assert_eq!(
            i.text,
            "Answered; the agent gets it when a slot is free (#2 in line)"
        );
        assert!(i.answers.is_empty() && i.choices.is_empty());
    }
}

#[test]
fn the_row_leaves_when_delivered_and_an_unanswered_paused_question_stays_a_person_item() {
    // Delivered: the agent runs again, nothing is paused.
    let mut delivered = asking(1, false);
    delivered.status = TicketStatus::Running;
    delivered.status_reason = None;
    assert!(with_line(vec![delivered], &[]).is_empty());
    // Control: paused and not answered yet is still the person's question.
    let open = paused(asking(2, false), false);
    let got = with_line(vec![open], &[]);
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].kind, InboxKind::Question);
    assert!(got[0].needs_human);
}

#[test]
fn informational_rows_come_after_the_persons_items() {
    let answered = paused(asking(1, false), true);
    let mut newer = asking(2, false);
    newer.status_since = Timestamp(1);
    let mut later_answered = answered;
    later_answered.status_since = Timestamp(900);
    let got = with_line(vec![later_answered, newer], &[(0, 1)]);
    let kinds: Vec<_> = got.iter().map(|i| (i.kind, i.needs_human)).collect();
    assert_eq!(
        kinds,
        [(InboxKind::Question, true), (InboxKind::Answered, false)]
    );
}

#[test]
fn without_a_known_place_the_sentence_has_no_number() {
    let got = with_line(vec![paused(asking(1, false), true)], &[]);
    assert_eq!(
        got[0].text,
        "Answered; the agent gets it when a slot is free"
    );
}
