//! The person's three decisions about leftovers (open subtasks of a card that later cards were
//! done before): move them into a new card at the end of the chain, close them, or leave them.
//! The rules are core's (`toknila_core::inbox::leftovers`); this module applies them.

use std::collections::HashMap;
use std::str::FromStr;

use toknila_core::events::DomainEvent;
use toknila_core::inbox::leftovers as rules;
use toknila_core::machine::person_only;
use toknila_core::{Actor, CommentBody, CommentKind, Subtask, Ticket, TicketKey};

use crate::inbox::project_tickets;
use crate::notes::add_comment;
use crate::tickets::{self, NewTicket};
use crate::{AppError, AppResult, UnitOfWork, with_chain};

/// What a person decides about a card's leftovers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeftoverAction {
    /// Open subtasks go to a new card at the end of the chain.
    Move,
    /// Open subtasks are marked done, with a note in the journal.
    Close,
    /// Nothing changes; the item leaves the inbox for good.
    Leave,
}

impl FromStr for LeftoverAction {
    type Err = AppError;

    fn from_str(s: &str) -> AppResult<Self> {
        Ok(match s {
            "move" => Self::Move,
            "close" => Self::Close,
            "leave" => Self::Leave,
            other => return Err(AppError::not_found(format!("leftover action {other}"))),
        })
    }
}

/// Applies a person's decision to the leftovers of `card`.
pub fn resolve<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    card: &Ticket,
    action: LeftoverAction,
) -> AppResult<()> {
    person_only(uow.actor(), "decide leftovers")?;
    let chain = uow
        .chain(card.chain_id)?
        .ok_or_else(|| AppError::not_found("chain"))?;
    let all = project_tickets(uow, chain.project_id)?;
    let checklist = uow.subtasks(card.id)?;
    rules::require_pending(&all, &checklist, card)?;
    match action {
        LeftoverAction::Move => move_to_new_card(uow, card, &checklist),
        LeftoverAction::Close => close(uow, card, &checklist),
        LeftoverAction::Leave => leave(uow, card, &all),
    }
}

fn note<U: UnitOfWork + ?Sized>(uow: &mut U, card: &Ticket, body: CommentBody) -> AppResult<()> {
    let view = uow
        .chain_view(card.chain_id)?
        .ok_or_else(|| AppError::not_found("chain"))?;
    add_comment(
        uow,
        &view,
        card.id,
        Actor::Human,
        CommentKind::Decision,
        body,
    )?;
    Ok(())
}

fn save<U: UnitOfWork + ?Sized>(uow: &mut U, card: &TicketKey, s: &Subtask) -> AppResult<()> {
    uow.upsert_subtask(s)?;
    uow.emit(&DomainEvent::SubtaskUpdated {
        ticket: card.clone(),
        subtask: s.clone(),
    })
}

fn move_to_new_card<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    card: &Ticket,
    checklist: &[Subtask],
) -> AppResult<()> {
    let view = uow
        .chain_view(card.chain_id)?
        .ok_or_else(|| AppError::not_found("chain"))?;
    let after = rules::chain_end(&view)
        .into_iter()
        .filter_map(|id| view.tickets.iter().find(|t| t.id == id))
        .map(|t| t.key.to_string())
        .collect();
    let input = NewTicket {
        title: rules::new_card_title(card),
        prompt: rules::new_card_prompt(card),
        after,
        ..NewTicket::default()
    };
    let id = tickets::create(uow, &card.chain_id.to_string(), input)?;
    let new_card = uow
        .ticket(id)?
        .ok_or_else(|| AppError::not_found("ticket"))?;
    let split = rules::move_out(checklist, id);
    let before: HashMap<_, _> = checklist.iter().map(|s| (s.id, s.position)).collect();
    for s in &split.moved {
        save(uow, &new_card.key, s)?;
    }
    for s in split
        .kept
        .iter()
        .filter(|s| before.get(&s.id) != Some(&s.position))
    {
        save(uow, &card.key, s)?;
    }
    note(
        uow,
        card,
        rules::moving_note(card, &new_card.key, &split.moved),
    )
}

fn close<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    card: &Ticket,
    checklist: &[Subtask],
) -> AppResult<()> {
    let closed = rules::close(checklist, uow.actor());
    for s in &closed {
        save(uow, &card.key, s)?;
    }
    note(uow, card, rules::closing_note(card, &closed))
}

/// Records the dismissal on every card whose mark still waits for a decision about `card`.
fn leave<U: UnitOfWork + ?Sized>(uow: &mut U, card: &Ticket, all: &[Ticket]) -> AppResult<()> {
    for mark in rules::marks_awaiting(all, &card.key) {
        with_chain(uow, mark.chain_id, |ctx| {
            Ok::<_, AppError>(ctx.edit_ticket(mark.id, |t| {
                if let Some(m) = t.done_out_of_order.as_mut() {
                    m.dismiss(&card.key);
                }
            })?)
        })?;
    }
    Ok(())
}
