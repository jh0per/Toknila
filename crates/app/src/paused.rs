//! A card that waits for a person holds no slot: after a grace the host ends its agent's process
//! and the card keeps what a run needs to come back (`Ticket::agent_paused`). Core decides what
//! pausing, wanting and resuming do (`toknila_core::machine`); the answer paths call
//! [`crate::tickets::comment`], which marks the card as wanting its agent back, and the schedule
//! starts it when a slot is free ([`crate::runs::queue_resume`]).

use toknila_core::machine::TicketEvent;
use toknila_core::question::AnswerKind;
use toknila_core::{
    CommentBody, CommentKind, StatusReason, Ticket, TicketId, TicketStatus, WaitingReason,
};

use crate::chain::tell_ticket;
use crate::{AppResult, UnitOfWork};

/// The host ended the process of a Waiting card's agent: the card keeps `session` and frees its
/// slot. A card that is no longer Waiting is left as it is.
pub fn pause<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    ticket: TicketId,
    session: Option<String>,
) -> AppResult<()> {
    tell_ticket(uow, ticket, TicketEvent::AgentPaused { session })
}

/// A person answers a permission the paused agent asked for. Its dialog went with its process, so
/// the answer is told to the agent as an instruction, which starts it again. `Ok(false)` when the
/// card's agent is not paused (the live dialog takes the answer).
pub fn answer_permission<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    ticket: &Ticket,
    answer: AnswerKind,
    text: Option<&str>,
) -> AppResult<bool> {
    let Some(t) = uow.ticket(ticket.id)? else {
        return Ok(false);
    };
    if t.status != TicketStatus::Waiting || t.agent_paused.is_none() {
        return Ok(false);
    }
    let asked = match &t.status_reason {
        Some(StatusReason::Waiting(WaitingReason::Permission { text, detail })) => {
            detail.clone().unwrap_or_else(|| text.clone())
        }
        _ => "the action you asked about".to_owned(),
    };
    let verdict = match answer {
        AnswerKind::Allow => Some("Allowed"),
        AnswerKind::Deny => Some("Denied"),
        _ => None,
    };
    let why = text.map(str::trim).filter(|s| !s.is_empty());
    let gist = match (verdict, why) {
        (Some(v), Some(why)) => format!("{v}: {asked}. {why}"),
        (Some(v), None) => format!("{v}: {asked}."),
        (None, Some(why)) => why.to_owned(),
        (None, None) => "Continue.".to_owned(),
    };
    let body = CommentBody {
        gist,
        ..CommentBody::default()
    };
    crate::tickets::comment(uow, &t, CommentKind::Instruction, body)?;
    Ok(true)
}
