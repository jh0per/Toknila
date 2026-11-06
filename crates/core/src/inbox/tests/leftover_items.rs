//! The `leftovers` item: earlier cards with open subtasks behind a card done out of order.

use super::*;
use crate::{OutOfOrder, SubtaskId};

fn sub(card: &Ticket, position: u32, text: &str, done: bool) -> Subtask {
    Subtask {
        id: SubtaskId::new(),
        ticket_id: card.id,
        position,
        text: text.to_owned(),
        done,
        done_by: done.then_some(Actor::Human),
    }
}

/// DEMO-1 is open with one done and two open subtasks; DEMO-2 and DEMO-3 were done behind it.
fn imported() -> (Vec<Ticket>, Vec<Subtask>) {
    let a = ticket(1, TicketStatus::Ready, 100);
    let mut b = ticket(2, TicketStatus::Merged, 200);
    let mut c = ticket(3, TicketStatus::Merged, 300);
    b.done_out_of_order = Some(OutOfOrder::new(vec![a.key.clone()]));
    c.done_out_of_order = Some(OutOfOrder::new(vec![a.key.clone()]));
    let subs = vec![
        sub(&a, 1, "T001 set up", true),
        sub(&a, 2, "T002 wire it", false),
        sub(&a, 3, "T003 test it", false),
    ];
    (vec![a, b, c], subs)
}

fn leftovers(tickets: Vec<Ticket>, subtasks: Vec<Subtask>) -> Vec<InboxItem> {
    compute(&InboxData {
        tickets,
        subtasks,
        ..InboxData::default()
    })
}

#[test]
fn an_earlier_card_with_open_subtasks_is_one_item_naming_the_done_cards() {
    let (tickets, subs) = imported();
    let got = leftovers(tickets, subs);
    assert_eq!(got.len(), 1);
    let i = &got[0];
    assert_eq!(i.kind, InboxKind::Leftovers);
    assert_eq!(i.ticket.to_string(), "DEMO-1");
    assert_eq!(i.subtasks, ["T002 wire it", "T003 test it"]);
    let done: Vec<_> = i.done_cards.iter().map(ToString::to_string).collect();
    assert_eq!(done, ["DEMO-2", "DEMO-3"]);
    assert_eq!(i.since, Timestamp(300));
}

#[test]
fn nothing_is_listed_once_the_subtasks_are_done_or_the_leftovers_are_dismissed() {
    let (tickets, mut subs) = imported();
    for s in &mut subs {
        s.done = true;
    }
    assert_eq!(leftovers(tickets, subs), []);

    let (mut tickets, subs) = imported();
    let a = tickets[0].key.clone();
    for t in &mut tickets[1..] {
        t.done_out_of_order.as_mut().expect("marked").dismiss(&a);
    }
    assert_eq!(leftovers(tickets, subs), []);
}

#[test]
fn one_dismissed_mark_does_not_hide_a_mark_that_still_waits() {
    let (mut tickets, subs) = imported();
    let a = tickets[0].key.clone();
    tickets[1]
        .done_out_of_order
        .as_mut()
        .expect("marked")
        .dismiss(&a);
    let got = leftovers(tickets, subs);
    assert_eq!(got.len(), 1);
    let done: Vec<_> = got[0].done_cards.iter().map(ToString::to_string).collect();
    assert_eq!(done, ["DEMO-3"]);
}

#[test]
fn a_card_without_a_mark_behind_it_is_not_a_leftover() {
    let (mut tickets, subs) = imported();
    for t in &mut tickets {
        t.done_out_of_order = None;
    }
    assert_eq!(leftovers(tickets, subs), []);
}
