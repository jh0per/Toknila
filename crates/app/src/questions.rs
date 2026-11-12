//! Questions that live with a run and not in the card's status: journaling that one was raised or
//! went away, and what a person's answer does to the card. What is asked, what may answer it and
//! how the answer is delivered is core's (`toknila_core::question`); the daemon detects and
//! delivers.

use toknila_core::events::DomainEvent;
use toknila_core::machine::{TicketEvent, person_only};
use toknila_core::question::{AnswerKind, QuestionKind};
use toknila_core::{AgentRole, StatusReason, Ticket, TicketStatus, WaitingReason};

use crate::chain::tell_ticket;
use crate::{AppError, AppResult, UnitOfWork, with_chain};

/// A run of `t` raised a question that no status shows (a reviewer's dialog); `text` says what is
/// asked in words, for the journal.
pub fn raised<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    t: &Ticket,
    role: AgentRole,
    kind: QuestionKind,
    text: Option<String>,
) -> AppResult<()> {
    uow.emit(&DomainEvent::QuestionRaised {
        key: t.key.clone(),
        ticket: t.id,
        role,
        kind,
        text,
    })
}

/// A run's own screen asks (`screen_lines` are what it shows): the journal says what the first
/// line is, and a worker's Running card moves to Waiting (reason: its screen), so the window and
/// `tok ls` say it waits for a person. Nothing but a person answers it.
pub fn screen_raised<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    t: &Ticket,
    role: AgentRole,
    screen_lines: &[String],
) -> AppResult<()> {
    let words = toknila_core::question::screen_words(screen_lines);
    raised(uow, t, role, QuestionKind::Screen, Some(words))?;
    let running = uow
        .ticket(t.id)?
        .is_some_and(|t| t.status == TicketStatus::Running);
    if role == AgentRole::Worker && running {
        let reason = WaitingReason::Screen;
        tell_ticket(uow, t.id, TicketEvent::AskHuman { reason })?;
    }
    Ok(())
}

/// The screen question went away without an answer (the run showed its first sign of life, or
/// ended): a worker's card that waited on it goes on.
pub fn screen_cleared<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    t: &Ticket,
    role: AgentRole,
) -> AppResult<()> {
    cleared(uow, t, role)?;
    let waiting = uow.ticket(t.id)?.is_some_and(|t| {
        t.status == TicketStatus::Waiting
            && t.status_reason == Some(StatusReason::Waiting(WaitingReason::Screen))
    });
    if role == AgentRole::Worker && waiting {
        tell_ticket(uow, t.id, TicketEvent::AgentActive)?;
    }
    Ok(())
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
