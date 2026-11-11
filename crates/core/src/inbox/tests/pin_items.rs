//! The `design_question` item: open pins of the assistant, listed under their card.

use super::*;
use crate::design::Region;
use crate::design::pin::DesignPin;
use crate::{DesignId, PinId, ProjectId};

fn pin(card: Option<&Ticket>, text: &str, answer: Option<&str>) -> DesignPin {
    DesignPin {
        id: PinId::new(),
        project: ProjectId::new(),
        design: DesignId::new(),
        design_name: "home.png".to_owned(),
        number: 1,
        region: Region {
            x: 1,
            y: 2,
            w: 3,
            h: 4,
        },
        text: text.to_owned(),
        card: card.map(|t| t.key.clone()),
        answer: answer.map(str::to_owned),
        created_at: Timestamp(500),
        answered_at: None,
    }
}

fn with_pins(tickets: Vec<Ticket>, pins: Vec<DesignPin>) -> Vec<InboxItem> {
    compute(&InboxData {
        tickets,
        pins,
        ..InboxData::default()
    })
}

#[test]
fn an_open_pin_is_a_design_question_of_its_card() {
    let card = ticket(1, TicketStatus::Ready, 100);
    let p = pin(Some(&card), "Is the logo fixed?", None);
    let out = with_pins(vec![card.clone()], vec![p.clone()]);
    assert_eq!(out.len(), 1);
    let i = &out[0];
    assert_eq!(i.kind, InboxKind::DesignQuestion);
    assert_eq!(i.ticket, Some(card.key.clone()));
    assert_eq!(i.project, Some(p.project));
    assert_eq!(i.text, "home.png: Is the logo fixed?");
    assert_eq!(i.design.as_deref(), Some("home.png"));
    assert_eq!(i.pin.as_deref(), Some(p.id.to_string().as_str()));
    assert_eq!(i.since, Timestamp(500));
}

#[test]
fn an_answered_pin_is_not_listed() {
    let card = ticket(1, TicketStatus::Ready, 100);
    let p = pin(Some(&card), "Is the logo fixed?", Some("Yes"));
    assert!(with_pins(vec![card], vec![p]).is_empty());
}

#[test]
fn an_open_pin_without_a_card_is_an_item_of_kind_pin() {
    let card = ticket(1, TicketStatus::Ready, 100);
    let p = pin(None, "Which logo?", None);
    let out = with_pins(vec![card], vec![p.clone()]);
    assert_eq!(out.len(), 1);
    let i = &out[0];
    assert_eq!(i.kind, InboxKind::Pin);
    assert_eq!((i.ticket.clone(), i.chain), (None, None));
    assert_eq!(i.project, Some(p.project));
    assert_eq!(i.title, "home.png");
    assert_eq!(i.text, "Which logo?");
    assert_eq!(i.pin.as_deref(), Some(p.id.to_string().as_str()));
    assert_eq!(i.design.as_deref(), Some("home.png"));
    assert_eq!(i.since, Timestamp(500));
    // Nothing to do to a card: only the answer settles it.
    assert!(i.allowed_actions.is_empty());
}

#[test]
fn a_pin_whose_card_is_not_in_the_project_is_a_pin_item_too() {
    let card = ticket(1, TicketStatus::Ready, 100);
    let other = ticket(9, TicketStatus::Ready, 100);
    let out = with_pins(vec![card], vec![pin(Some(&other), "Other?", None)]);
    assert_eq!(
        out.iter().map(|i| i.kind).collect::<Vec<_>>(),
        [InboxKind::Pin]
    );
}

#[test]
fn an_answered_pin_without_a_card_is_not_listed() {
    let p = pin(None, "Which logo?", Some("The blue one"));
    assert!(with_pins(vec![], vec![p]).is_empty());
}
