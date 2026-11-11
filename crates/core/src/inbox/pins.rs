//! The `design_question` items: the assistant's open pins on a design, one per pin, listed under
//! the card they are about.

use super::{InboxData, InboxItem, InboxKind, item};

/// One item per open pin whose card is in the project. A pin without a card has no chain to
/// list it under; it shows on the mockup and in the plan only.
pub(super) fn pin_items(data: &InboxData) -> Vec<InboxItem> {
    data.pins
        .iter()
        .filter(|p| p.is_open())
        .filter_map(|p| {
            let card = data
                .tickets
                .iter()
                .find(|t| Some(&t.key) == p.card.as_ref())?;
            let mut i = item(
                InboxKind::DesignQuestion,
                card,
                p.created_at,
                format!("{}: {}", p.design_name, p.text),
            );
            i.pin = Some(p.id.to_string());
            i.design = Some(p.design_name.clone());
            Some(i)
        })
        .collect()
}
