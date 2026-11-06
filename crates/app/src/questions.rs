//! Questions that live with a run and not in the card's status: journaling that one was raised or
//! went away, and what a person's answer does to the card. What is asked, what may answer it and
//! how the answer is delivered is core's (`toknila_core::question`); the daemon detects and
//! delivers.

use toknila_core::events::DomainEvent;
use toknila_core::machine::{TicketEvent, person_only};
use toknila_core::question::{AnswerKind, QuestionKind};
use toknila_core::{AgentRole, Ticket, TicketStatus};

use crate::{AppError, AppResult, UnitOfWork, with_chain};

/// A run of `t` raised a question that no status shows (a reviewer's dialog, a screen wait).
pub fn raised<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    t: &Ticket,
    role: AgentRole,
    kind: QuestionKind,
) -> AppResult<()> {
    uow.emit(&DomainEvent::QuestionRaised {
        key: t.key.clone(),
        ticket: t.id,
        role,
        kind,
    })
}

/// Such a question went away without an answer.
pub fn cleared<U: UnitOfWork + ?Sized>(uow: &mut U, t: &Ticket, role: AgentRole) -> AppResult<()> {
    uow.emit(&DomainEvent::QuestionCleared {
        key: t.key.clone(),
        ticket: t.id,
        role,
    })
}

/// A person's answer reached the run of `role`. The journal says who answered and whose run it
/// was; a worker that waits goes back to Running at once (machine event `Answered`) instead of
/// when its next tool call ends.
pub fn answered<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    t: &Ticket,
    role: AgentRole,
    answer: AnswerKind,
) -> AppResult<()> {
    person_only(uow.actor(), "answer an agent")?;
    uow.emit(&DomainEvent::QuestionAnswered {
        key: t.key.clone(),
        ticket: t.id,
        role,
        answer,
    })?;
    let waiting = role == AgentRole::Worker
        && uow
            .ticket(t.id)?
            .is_some_and(|t| t.status == TicketStatus::Waiting);
    if waiting {
        with_chain(uow, t.chain_id, |ctx| {
            Ok::<_, AppError>(ctx.ticket_event(t.id, TicketEvent::Answered)?)
        })?;
    }
    Ok(())
}
