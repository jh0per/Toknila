//! Comments and subtasks: what humans and agents write on a ticket.

use toknila_core::events::DomainEvent;
use toknila_core::graph::ChainView;
use toknila_core::machine::TicketEvent;
use toknila_core::{
    Actor, AgentProfile, Comment, CommentBody, CommentId, CommentKind, NodeRef, Subtask, SubtaskId,
    Ticket, TicketId, TicketStatus,
};

use crate::{AppError, AppResult, ChainTx, UnitOfWork};

/// First line becomes the gist, the rest the "what" field.
pub fn body_from_text(text: &str) -> CommentBody {
    let text = text.trim();
    let (gist, rest) = text.split_once('\n').unwrap_or((text, ""));
    let rest = rest.trim();
    CommentBody {
        gist: gist.trim().to_owned(),
        what: (!rest.is_empty()).then(|| rest.to_owned()),
        ..CommentBody::default()
    }
}

/// Stores a comment and journals it.
pub fn add_comment<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    view: &ChainView,
    ticket: TicketId,
    author: Actor,
    kind: CommentKind,
    body: CommentBody,
) -> AppResult<Comment> {
    let body = toknila_core::template::normalize(&body);
    if body.gist.is_empty() {
        return Err(AppError::bad_request(
            "comment needs a gist (first line of text)",
        ));
    }
    let c = Comment {
        id: CommentId::new(),
        ticket_id: ticket,
        author,
        kind,
        body,
        created_at: uow.now(),
        delivered_at: None,
    };
    uow.upsert_comment(&c)?;
    uow.emit(&DomainEvent::CommentAdded {
        ticket: view.label(NodeRef::Ticket(ticket)),
        comment: c.clone(),
    })?;
    Ok(c)
}

/// Inserts a subtask at the end of a ticket's checklist.
pub fn add_subtask<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    ticket: &Ticket,
    text: &str,
) -> AppResult<Subtask> {
    let text = text.trim();
    if text.is_empty() {
        return Err(AppError::bad_request("subtask text is empty"));
    }
    let position = u32::try_from(uow.subtasks(ticket.id)?.len())
        .unwrap_or(u32::MAX)
        .saturating_add(1);
    let s = Subtask {
        id: SubtaskId::new(),
        ticket_id: ticket.id,
        position,
        text: text.to_owned(),
        done: false,
        done_by: None,
    };
    uow.upsert_subtask(&s)?;
    uow.emit(&DomainEvent::SubtaskAdded {
        ticket: ticket.key.clone(),
        subtask: s.clone(),
    })?;
    Ok(s)
}

/// Marks a subtask done or not, or renames it.
pub fn update_subtask<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    ticket: &Ticket,
    position: u32,
    done: Option<bool>,
    text: Option<String>,
) -> AppResult<Subtask> {
    let mut s = uow
        .subtasks(ticket.id)?
        .into_iter()
        .find(|s| s.position == position)
        .ok_or_else(|| AppError::not_found(format!("subtask {position} of {}", ticket.key)))?;
    if let Some(done) = done {
        s.done = done;
        s.done_by = done.then_some(uow.actor());
    }
    if let Some(text) = text {
        s.text = text;
    }
    uow.upsert_subtask(&s)?;
    uow.emit(&DomainEvent::SubtaskUpdated {
        ticket: ticket.key.clone(),
        subtask: s.clone(),
    })?;
    Ok(s)
}

/// Simulated agents read instructions instantly: pending human comments on a Running or Waiting
/// sim ticket are marked delivered, and a Waiting ticket goes back to Running.
/// Returns whether anything changed.
pub(crate) fn deliver_to_sim<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    ctx: &mut ChainTx,
) -> AppResult<bool> {
    let default_profile = uow
        .project(ctx.view.chain.project_id)?
        .map(|p| p.default_profile)
        .unwrap_or_else(|| AgentProfile::SIM.to_owned());
    let candidates: Vec<(TicketId, TicketStatus)> = ctx
        .view
        .tickets
        .iter()
        .filter(|t| matches!(t.status, TicketStatus::Running | TicketStatus::Waiting))
        .filter(|t| t.profile.as_deref().unwrap_or(&default_profile) == AgentProfile::SIM)
        .map(|t| (t.id, t.status))
        .collect();
    let mut changed = false;
    for (ticket, status) in candidates {
        let pending: Vec<Comment> = uow
            .comments(ticket)?
            .into_iter()
            .filter(|c| {
                c.author == Actor::Human
                    && c.kind == CommentKind::Instruction
                    && c.delivered_at.is_none()
            })
            .collect();
        if pending.is_empty() {
            continue;
        }
        for mut c in pending {
            c.delivered_at = Some(uow.now());
            uow.upsert_comment(&c)?;
            uow.emit(&DomainEvent::CommentDelivered {
                ticket: ctx.view.label(NodeRef::Ticket(ticket)),
                comment: c,
            })?;
        }
        if status == TicketStatus::Waiting {
            ctx.ticket_event(ticket, TicketEvent::InstructionDelivered)?;
        }
        changed = true;
    }
    Ok(changed)
}
