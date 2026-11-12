//! Getting a person's instruction comments to a live agent. Generic over any instruction comment
//! a person leaves on a ticket (a typed one, a Return), whoever wrote it and why.
//!
//! A new instruction on a ticket whose agent is a process is queued as an external effect. The
//! host picks the way in (`toknila_core::instructions::route`): typing it into the terminal now,
//! or leaving it pending for the agent's next Stop hook, which [`claim`]s it. Either way a comment
//! is delivered once: the delivered mark, the journal event and the Waiting-to-Running move all
//! happen when it is marked, and a delivered comment is never offered again.

use toknila_core::events::DomainEvent;
use toknila_core::graph::ChainView;
use toknila_core::instructions;
use toknila_core::machine::TicketEvent;
use toknila_core::{
    Actor, AgentProfile, Comment, CommentId, CommentKind, NodeRef, StatusReason, Ticket, TicketId,
    WaitingReason,
};

use crate::chain::tell_ticket;
use crate::external::ExternalEffect;
use crate::{AppResult, UnitOfWork};

/// A person's instruction that no agent has seen yet, oldest first.
fn pending<U: UnitOfWork + ?Sized>(uow: &U, ticket: TicketId) -> AppResult<Vec<Comment>> {
    Ok(uow
        .comments(ticket)?
        .into_iter()
        .filter(|c| c.kind == CommentKind::Instruction && c.delivered_at.is_none())
        .collect())
}

/// Queues delivery of a comment that was just added, if it is an instruction from a person to a
/// ticket whose agent is a process. Whether that process is alive is the host's to find out; if
/// it is not, the comment stays pending.
pub(crate) fn queue<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    view: &ChainView,
    comment: &Comment,
) -> AppResult<()> {
    // A person's instruction, a Return's comment, or the reviewer agent's remarks: every
    // instruction comment is for the worker, whoever wrote it.
    if comment.kind != CommentKind::Instruction {
        return Ok(());
    }
    let Some(ticket) = view.tickets.iter().find(|t| t.id == comment.ticket_id) else {
        return Ok(());
    };
    let Some(project) = uow.project(view.chain.project_id)? else {
        return Ok(());
    };
    // A paused agent has no terminal to type into: the answer starts it again (`tickets::comment`).
    if project.profile_for(ticket) != AgentProfile::SIM && ticket.agent_paused.is_none() {
        uow.defer(ExternalEffect::DeliverInstruction {
            ticket: ticket.id,
            comment: comment.clone(),
        });
    }
    Ok(())
}

/// Marks the comments delivered: the mark, the journal event, and a Waiting ticket goes back to
/// Running.
fn mark<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    ticket: &Ticket,
    comments: Vec<Comment>,
) -> AppResult<()> {
    if comments.is_empty() {
        return Ok(());
    }
    let comments_were_nudges = comments.iter().all(|c| c.author == Actor::System);
    let label = uow
        .chain_view(ticket.chain_id)?
        .map(|v| v.label(NodeRef::Ticket(ticket.id)));
    for mut c in comments {
        c.delivered_at = Some(uow.now());
        uow.upsert_comment(&c)?;
        if let Some(label) = &label {
            uow.emit(&DomainEvent::CommentDelivered {
                ticket: label.clone(),
                comment: c,
            })?;
        }
    }
    // A nudge for silence says nothing to a question the agent has open: it stays open.
    let only_nudges = comments_were_nudges;
    let asks = uow.ticket(ticket.id)?.is_some_and(|t| {
        matches!(
            t.status_reason,
            Some(StatusReason::Waiting(
                WaitingReason::Question { .. } | WaitingReason::Permission { .. }
            ))
        )
    });
    if only_nudges && asks {
        return Ok(());
    }
    tell_ticket(uow, ticket.id, TicketEvent::InstructionDelivered)
}

/// What the Stop hook gets: the pending instructions as one text, now marked delivered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Claimed {
    /// The text for the agent; empty when nothing was pending.
    pub reason: String,
    pub count: usize,
}

/// The agent's Stop hook asks for what a person said while it worked. Each instruction is handed
/// out once: claiming marks it delivered.
pub fn claim<U: UnitOfWork + ?Sized>(uow: &mut U, ticket: &Ticket) -> AppResult<Claimed> {
    let comments = pending(uow, ticket.id)?;
    let claimed = Claimed {
        reason: instructions::for_hook(&comments),
        count: comments.len(),
    };
    mark(uow, ticket, comments)?;
    Ok(claimed)
}

/// A new run started with these pending comments in its kickoff and instructions file: they are
/// delivered, so the Stop hook does not hand them out again. The card's status is not touched:
/// the run's start is what moved it.
pub(crate) fn read_at_start<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    ticket: TicketId,
    comments: &[CommentId],
) -> AppResult<()> {
    let Some(t) = uow.ticket(ticket)? else {
        return Ok(());
    };
    let label = uow
        .chain_view(t.chain_id)?
        .map(|v| v.label(NodeRef::Ticket(ticket)));
    for mut c in pending(uow, ticket)? {
        if !comments.contains(&c.id) {
            continue;
        }
        c.delivered_at = Some(uow.now());
        uow.upsert_comment(&c)?;
        if let Some(label) = &label {
            uow.emit(&DomainEvent::CommentDelivered {
                ticket: label.clone(),
                comment: c,
            })?;
        }
    }
    Ok(())
}

/// The host typed the comment into the terminal: it is delivered now.
pub(crate) fn typed<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    ticket: TicketId,
    comment: CommentId,
) -> AppResult<()> {
    let Some(t) = uow.ticket(ticket)? else {
        return Ok(());
    };
    let found: Vec<Comment> = pending(uow, ticket)?
        .into_iter()
        .filter(|c| c.id == comment)
        .collect();
    mark(uow, &t, found)
}
