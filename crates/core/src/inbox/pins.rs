//! The pin items: the assistant's open pins on a design. A pin about a card of the project is a
//! `design_question`, listed under the card; a pin with no card (or whose card is gone) is a
//! `pin`, listed under its design. Both are answered in place.

use super::{InboxData, InboxItem, InboxKind, item};
use crate::design::pin::DesignPin;

/// One item per open pin.
pub(super) fn pin_items(data: &InboxData) -> Vec<InboxItem> {
    data.pins
        .iter()
        .filter(|p| p.is_open())
        .map(|p| {
            let card = data
                .tickets
                .iter()
                .find(|t| Some(&t.key) == p.card.as_ref());
            let mut i = match card {
                Some(card) => item(
                    InboxKind::DesignQuestion,
                    card,
                    p.created_at,
                    format!("{}: {}", p.design_name, p.text),
                ),
                None => cardless(p),
            };
            i.project = Some(p.project);
            i.pin = Some(p.id.to_string());
            i.design = Some(p.design_name.clone());
            i
        })
        .collect()
}

/// The item of a pin that no card of the project owns: the design is its title, the question its
/// text, and there is no card to open.
fn cardless(p: &DesignPin) -> InboxItem {
    InboxItem {
        kind: InboxKind::Pin,
        ticket: None,
        chain: None,
        project: None,
        title: p.design_name.clone(),
        since: p.created_at,
        text: p.text.clone(),
        detail: None,
        thread: None,
        subtasks: Vec::new(),
        done_cards: Vec::new(),
        link: None,
        allowed_actions: Vec::new(),
        role: None,
        answers: Vec::new(),
        lines: Vec::new(),
        pin: None,
        allow_always: None,
        allow_always_none: None,
        hand_in: None,
        design: None,
    }
}
