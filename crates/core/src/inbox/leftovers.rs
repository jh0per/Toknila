//! Leftovers: open subtasks of cards that were left behind when a later card was imported as
//! done. A person decides once per card: move them into a new card at the end of the chain,
//! close them, or leave them.

use crate::graph::ChainView;
use crate::{Actor, CommentBody, NodeRef, Subtask, Ticket, TicketId, TicketKey};

/// Why a leftover action was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LeftoverError {
    #[error("{0} has no leftovers waiting for a decision")]
    NothingLeft(TicketKey),
}

/// The open subtasks of one card, in order.
pub fn open_subtasks(subtasks: &[Subtask], card: TicketId) -> Vec<Subtask> {
    let mut open: Vec<Subtask> = subtasks
        .iter()
        .filter(|s| s.ticket_id == card && !s.done)
        .cloned()
        .collect();
    open.sort_by_key(|s| s.position);
    open
}

/// The cards done out of order whose mark still waits for a decision about `card`.
pub fn marks_awaiting<'a>(tickets: &'a [Ticket], card: &TicketKey) -> Vec<&'a Ticket> {
    tickets
        .iter()
        .filter(|t| t.done_out_of_order.as_ref().is_some_and(|m| m.awaits(card)))
        .collect()
}

/// Checks that `card` has leftovers to decide about: an undismissed mark and open subtasks.
pub fn require_pending(
    tickets: &[Ticket],
    subtasks: &[Subtask],
    card: &Ticket,
) -> Result<(), LeftoverError> {
    if marks_awaiting(tickets, &card.key).is_empty() || open_subtasks(subtasks, card.id).is_empty()
    {
        return Err(LeftoverError::NothingLeft(card.key.clone()));
    }
    Ok(())
}

/// The title of the card that takes the leftovers of `card`.
pub fn new_card_title(card: &Ticket) -> String {
    format!("Leftovers from {}", card.title)
}

/// The prompt of the card that takes the leftovers of `card`.
pub fn new_card_prompt(card: &Ticket) -> String {
    format!(
        "Finish the tasks that were left open on {} ({}) when later stories were done before it.",
        card.title, card.key
    )
}

/// The tickets a card added at the end of the chain waits for: those no other ticket waits for.
/// A proposal nobody approved is not part of the plan and is left out.
pub fn chain_end(view: &ChainView) -> Vec<TicketId> {
    view.tickets
        .iter()
        .filter(|t| t.in_plan())
        .filter(|t| {
            !view
                .edges
                .iter()
                .any(|e| e.from == NodeRef::Ticket(t.id) && matches!(e.to, NodeRef::Ticket(_)))
        })
        .map(|t| t.id)
        .collect()
}

/// A checklist after leftovers moved out of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Moved {
    /// The open subtasks, now on the new card, in their order, text untouched.
    pub moved: Vec<Subtask>,
    /// What stays on the old card, numbered again from 1.
    pub kept: Vec<Subtask>,
}

/// Splits `checklist` (one card's subtasks) into the open ones, moved to card `to`, and the rest.
/// The rows keep their ids and text; only the card and the numbering change.
pub fn move_out(checklist: &[Subtask], to: TicketId) -> Moved {
    let mut all: Vec<&Subtask> = checklist.iter().collect();
    all.sort_by_key(|s| s.position);
    let (open, rest): (Vec<&Subtask>, Vec<&Subtask>) = all.into_iter().partition(|s| !s.done);
    let renumbered = |list: Vec<&Subtask>, ticket: Option<TicketId>| -> Vec<Subtask> {
        (1..)
            .zip(list)
            .map(|(position, s)| Subtask {
                position,
                ticket_id: ticket.unwrap_or(s.ticket_id),
                ..s.clone()
            })
            .collect()
    };
    Moved {
        moved: renumbered(open, Some(to)),
        kept: renumbered(rest, None),
    }
}

/// The open subtasks of `checklist`, marked done by `by`.
pub fn close(checklist: &[Subtask], by: Actor) -> Vec<Subtask> {
    let mut closed: Vec<Subtask> = checklist
        .iter()
        .filter(|s| !s.done)
        .map(|s| Subtask {
            done: true,
            done_by: Some(by),
            ..s.clone()
        })
        .collect();
    closed.sort_by_key(|s| s.position);
    closed
}

fn count(n: usize) -> String {
    format!("{n} leftover task{}", if n == 1 { "" } else { "s" })
}

fn bullets(subtasks: &[Subtask]) -> Option<String> {
    let lines: Vec<String> = subtasks.iter().map(|s| format!("- {}", s.text)).collect();
    (!lines.is_empty()).then(|| lines.join("\n"))
}

/// The journal note of a close.
pub fn closing_note(card: &Ticket, closed: &[Subtask]) -> CommentBody {
    CommentBody {
        gist: format!("Closed {} of {}", count(closed.len()), card.key),
        what: bullets(closed),
        why: Some("A person closed them from the inbox".to_owned()),
        ..CommentBody::default()
    }
}

/// The journal note of a move into `to`.
pub fn moving_note(card: &Ticket, to: &TicketKey, moved: &[Subtask]) -> CommentBody {
    CommentBody {
        gist: format!("Moved {} of {} to {to}", count(moved.len()), card.key),
        what: bullets(moved),
        why: Some("A person moved them from the inbox".to_owned()),
        ..CommentBody::default()
    }
}

#[cfg(test)]
mod tests;
