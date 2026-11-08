//! Agent events: what a runner reports about its ticket or milestone, turned into transitions.

use toknila_core::agent::{AgentEvent, ReleaseEvent};
use toknila_core::graph::ChainView;
use toknila_core::machine::TicketEvent;
use toknila_core::nudge;
use toknila_core::{
    Actor, BlockedReason, CommentBody, CommentKind, Release, Ticket, TicketId, TicketStatus,
};

use crate::notes::{add_comment, body_from_text, update_subtask};
use crate::{AppError, AppResult, UnitOfWork, reviews, threads, wildcards, with_chain};

/// An agent holds a grant for one ticket and acts on that ticket only.
pub fn ensure_granted(granted: TicketId, t: &Ticket) -> AppResult<()> {
    if granted == t.id {
        Ok(())
    } else {
        Err(AppError::forbidden(format!(
            "this agent works on another ticket, not {}",
            t.key
        )))
    }
}

/// Applies one event an agent reported about `t`. Returns the ticket the answer is about: `t`,
/// or the new card for `propose`.
pub fn ticket_event<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    t: &Ticket,
    event: AgentEvent,
) -> AppResult<TicketId> {
    // A report of work shows the agent is working again. A question, a crash, and talk with the
    // owner (a proposal, a thread) do not: the card may well be waiting on the answer.
    let resumed = match event {
        AgentEvent::Started
        | AgentEvent::SubtaskDone { .. }
        | AgentEvent::Note { .. }
        | AgentEvent::Finished { .. } => resume(uow, t)?,
        _ => false,
    };
    match event {
        AgentEvent::Propose(card) => {
            let (chain, from) = (t.chain_id.to_string(), t.key.to_string());
            return wildcards::propose(uow, &chain, card, Some(&from));
        }
        AgentEvent::OpenThread { kind, title } => {
            threads::open(uow, t, kind, &title)?;
            Ok(())
        }
        AgentEvent::ThreadMessage { thread, text } => {
            let th = threads::find(uow, t, &thread)?;
            threads::post_message(uow, t, &th, &text).map(|_| ())
        }
        AgentEvent::ThreadVersion {
            thread,
            format,
            body,
        } => {
            let th = threads::find(uow, t, &thread)?;
            let input = threads::NewVersion {
                format,
                body,
                message: None,
            };
            threads::post_version(uow, t, &th, input).map(|_| ())
        }
        AgentEvent::Started => with_chain(uow, t.chain_id, |ctx| {
            if t.status == TicketStatus::Ready {
                ctx.ticket_event(t.id, TicketEvent::Start)?;
            } else if t.status != TicketStatus::Running && !resumed {
                return Err(AppError::conflict(format!(
                    "cannot start a ticket in {:?}",
                    t.status
                )));
            }
            Ok(())
        }),
        AgentEvent::AskHuman {
            text,
            permission,
            detail,
        } => {
            if text.trim().is_empty() {
                return Err(AppError::bad_request("ask_human needs a text"));
            }
            let reason = AgentEvent::waiting_reason(&text, permission, detail);
            with_chain(uow, t.chain_id, |ctx| {
                Ok::<_, AppError>(ctx.ticket_event(t.id, TicketEvent::AskHuman { reason })?)
            })?;
            // A permission request lives in the journal and the inbox; it is not a comment on
            // the card (a question the agent asks is).
            if permission {
                return Ok(t.id);
            }
            report(uow, t, body_from_text(&text))
        }
        AgentEvent::SubtaskDone { position } => {
            update_subtask(uow, t, position, Some(true), None).map(|_| ())
        }
        AgentEvent::Note { body } => report(uow, t, body),
        AgentEvent::Finished { summary } => {
            if summary.trim().is_empty() {
                return Err(AppError::bad_request("finished needs a summary"));
            }
            let open_threads = threads::open_threads(uow, t)?;
            if open_threads.is_empty() {
                threads::require_design(uow, t)?;
            }
            let mode = reviews::mode_of(uow, t)?;
            with_chain(uow, t.chain_id, |ctx| {
                let event = TicketEvent::Finished {
                    summary,
                    open_threads,
                    mode,
                };
                Ok(ctx.ticket_event(t.id, event)?)
            })
        }
        AgentEvent::ReviewVerdict { verdict, comment } => with_chain(uow, t.chain_id, |ctx| {
            let event = TicketEvent::ReviewVerdict { verdict, comment };
            Ok(ctx.ticket_event(t.id, event)?)
        }),
        AgentEvent::Crashed { detail } => with_chain(uow, t.chain_id, |ctx| {
            let reason = BlockedReason::Crashed { detail };
            Ok(ctx.ticket_event(t.id, TicketEvent::Block { reason })?)
        }),
    }
    .map(|()| t.id)
}

/// Applies one event a release agent reported about `r`.
pub fn release_event<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    r: &Release,
    event: ReleaseEvent,
) -> AppResult<()> {
    match event {
        ReleaseEvent::Finished => {
            with_chain(uow, r.chain_id, |ctx| Ok(ctx.complete_release(r.id)?))
        }
    }
}

fn report<U: UnitOfWork + ?Sized>(uow: &mut U, t: &Ticket, body: CommentBody) -> AppResult<()> {
    let view: ChainView = uow
        .chain_view(t.chain_id)?
        .ok_or_else(|| AppError::not_found("chain"))?;
    add_comment(uow, &view, t.id, Actor::Agent, CommentKind::Report, body).map(|_| ())
}

/// A sign of life from the agent (a hook fired). A card that waits goes back to Running when the
/// signal shows work (`toknila_core::nudge::resumes`); otherwise nothing about the ticket changes.
pub fn activity<U: UnitOfWork + ?Sized>(uow: &mut U, t: &Ticket, signal: &str) -> AppResult<()> {
    if nudge::resumes(signal) {
        resume(uow, t)?;
    }
    Ok(())
}

/// A Waiting ticket whose agent is active again goes back to Running, through the machine.
/// Returns whether it was Waiting.
fn resume<U: UnitOfWork + ?Sized>(uow: &mut U, t: &Ticket) -> AppResult<bool> {
    let waiting = uow
        .ticket(t.id)?
        .is_some_and(|t| t.status == TicketStatus::Waiting);
    if waiting {
        with_chain(uow, t.chain_id, |ctx| {
            Ok::<_, AppError>(ctx.ticket_event(t.id, TicketEvent::AgentActive)?)
        })?;
    }
    Ok(waiting)
}
