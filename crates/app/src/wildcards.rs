//! Wildcard cards: work nobody planned. An agent or a person proposes one; it waits in Proposed
//! until a person approves it (`TicketAction::Approve`) or rejects it (`TicketAction::Reject`).

use serde::{Deserialize, Serialize};
use toknila_core::agent::Proposal;
use toknila_core::graph::ChainView;
use toknila_core::{NodeRef, Ticket, TicketDraft, TicketId, TicketKey, Wildcard};

use crate::tickets::next_key;
use crate::{AppError, AppResult, UnitOfWork, resolve, with_chain};

/// `POST /v1/chains/{chain}/wildcards`: a person proposes a card.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct NewWildcard {
    #[serde(flatten)]
    pub card: Proposal,
    /// The ticket during which the work was found: key, id or title.
    pub from: Option<String>,
}

/// Proposes a wildcard card on a chain, by the command's actor. It lands in Proposed with its
/// dependencies pending; nothing else in the chain changes.
pub fn propose<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    chain: &str,
    card: Proposal,
    from: Option<&str>,
) -> AppResult<TicketId> {
    let title = card.title.trim();
    if title.is_empty() {
        return Err(AppError::bad_request("card title is empty"));
    }
    let c = resolve::chain(uow, chain)?;
    let key = next_key(uow, c.project_id)?;
    let draft = TicketDraft {
        title: title.to_owned(),
        prompt: card.prompt,
        acceptance: card.acceptance,
        ..TicketDraft::default()
    };
    let (id, by, now) = (TicketId::new(), uow.actor(), uow.now());
    with_chain(uow, c.id, move |ctx| -> AppResult<()> {
        let mark = Wildcard {
            from: from.map(|r| origin(&ctx.view, r)).transpose()?,
            proposed_by: by,
            after: nodes(&ctx.view, &card.after)?,
            before: nodes(&ctx.view, &card.before)?,
            approved: false,
        };
        let t = Ticket::new_proposed(id, c.id, key, draft, mark, now);
        Ok(ctx.propose(t)?)
    })?;
    Ok(id)
}

fn nodes(view: &ChainView, refs: &[String]) -> AppResult<Vec<NodeRef>> {
    refs.iter().map(|r| resolve::node(view, r)).collect()
}

/// The ticket a card was found during, as its key; it must be a ticket of the same chain.
fn origin(view: &ChainView, r: &str) -> AppResult<TicketKey> {
    match resolve::node(view, r)? {
        NodeRef::Ticket(id) => view
            .tickets
            .iter()
            .find(|t| t.id == id)
            .map(|t| t.key.clone())
            .ok_or_else(|| AppError::not_found(format!("ticket {r}"))),
        NodeRef::Release(_) => Err(AppError::bad_request(format!(
            "from must be a ticket, {r} is a release"
        ))),
    }
}
