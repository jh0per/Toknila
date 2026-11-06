//! Tickets: create and edit them, apply a person's actions, comment on them.

use serde::{Deserialize, Serialize};
pub use toknila_core::machine::TicketAction;
use toknila_core::machine::TicketEvent;
use toknila_core::{
    Actor, CommentBody, CommentKind, Edge, EdgeKind, NodeRef, ReviewMode, Ticket, TicketDraft,
    TicketId, TicketKey, TicketPatch,
};

use crate::notes::{add_comment, add_subtask, body_from_text};
use crate::{AppError, AppResult, UnitOfWork, resolve, with_chain};

/// `POST /v1/chains/{chain}/tickets`.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct NewTicket {
    pub title: String,
    #[serde(default)]
    pub prompt: String,
    #[serde(default)]
    pub acceptance: Vec<String>,
    /// Dependencies: ticket keys or release versions of the same chain.
    #[serde(default)]
    pub after: Vec<String>,
    /// Checklist items.
    #[serde(default)]
    pub subtasks: Vec<String>,
    pub profile: Option<String>,
    pub review_mode: Option<ReviewMode>,
    /// Reviewer agent profile for this ticket.
    pub reviewer: Option<String>,
    #[serde(default)]
    pub needs_secrets: bool,
    #[serde(default)]
    pub paused: bool,
}

/// Adds a ticket to a chain. The ticket and its edges land together, then the chain settles
/// (autostart); the checklist follows.
pub fn create<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    chain: &str,
    input: NewTicket,
) -> AppResult<TicketId> {
    let title = input.title.trim();
    if title.is_empty() {
        return Err(AppError::bad_request("ticket title is empty"));
    }
    let c = resolve::chain(uow, chain)?;
    let project = uow
        .project(c.project_id)?
        .ok_or_else(|| AppError::not_found("project"))?;
    let number = uow.take_ticket_number(project.id)?;
    let key = TicketKey::new(project.key.clone(), number)
        .map_err(|e| AppError::bad_request(e.to_string()))?;
    let draft = TicketDraft {
        title: title.to_owned(),
        prompt: input.prompt,
        acceptance: input.acceptance,
        paused: input.paused,
        profile: input.profile,
        review_mode: input.review_mode,
        reviewer: input.reviewer.filter(|r| !r.trim().is_empty()),
        needs_secrets: input.needs_secrets,
    };
    let t = Ticket::new_backlog(TicketId::new(), c.id, key, draft, uow.now());
    let id = t.id;
    let after = input.after;
    with_chain(uow, c.id, move |ctx| -> AppResult<()> {
        let deps = after
            .iter()
            .map(|a| resolve::node(&ctx.view, a))
            .collect::<AppResult<Vec<_>>>()?;
        ctx.add_ticket(t);
        for from in deps {
            ctx.add_edge(Edge {
                chain_id: ctx.view.chain.id,
                from,
                to: NodeRef::Ticket(id),
                kind: EdgeKind::Normal,
            })?;
        }
        Ok(())
    })?;
    let ticket = uow
        .ticket(id)?
        .ok_or_else(|| AppError::not_found("ticket"))?;
    for s in &input.subtasks {
        add_subtask(uow, &ticket, s)?;
    }
    Ok(id)
}

/// Changes a ticket's plain fields.
pub fn edit<U: UnitOfWork + ?Sized>(uow: &mut U, t: &Ticket, patch: TicketPatch) -> AppResult<()> {
    with_chain(uow, t.chain_id, |ctx| {
        Ok(ctx.edit_ticket(t.id, |t| patch.apply(t))?)
    })
}

/// Applies a state-machine event to a ticket.
pub fn act<U: UnitOfWork + ?Sized>(uow: &mut U, t: &Ticket, event: TicketEvent) -> AppResult<()> {
    with_chain(uow, t.chain_id, |ctx| Ok(ctx.ticket_event(t.id, event)?))
}

/// `POST /v1/tickets/{ticket}/comments`. Give `text` (first line is the gist, the rest becomes
/// "what") or a full `body`.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct NewComment {
    /// Defaults to instruction.
    pub kind: Option<CommentKind>,
    pub text: Option<String>,
    pub body: Option<CommentBody>,
}

impl NewComment {
    /// The comment's kind and body; a body or a text is required.
    pub fn into_parts(self) -> AppResult<(CommentKind, CommentBody)> {
        let body = match (self.body, self.text) {
            (Some(b), _) => b,
            (None, Some(text)) => body_from_text(&text),
            (None, None) => return Err(AppError::bad_request("give text or body")),
        };
        Ok((self.kind.unwrap_or(CommentKind::Instruction), body))
    }
}

/// A person's comment on a ticket. Delivery to a simulated agent happens as the chain settles.
pub fn comment<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    t: &Ticket,
    kind: CommentKind,
    body: CommentBody,
) -> AppResult<()> {
    let view = uow
        .chain_view(t.chain_id)?
        .ok_or_else(|| AppError::not_found("chain"))?;
    add_comment(uow, &view, t.id, Actor::Human, kind, body)?;
    with_chain(uow, t.chain_id, |_| Ok::<_, AppError>(()))
}
