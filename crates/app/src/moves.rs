//! Moves that touch two places and must land whole: a dependency to another end, a design
//! reference to another card. Each is one change of the chain, so a refusal leaves it as it was.

use toknila_core::{ChainId, Edge, EdgeKind, NodeRef};

use crate::chains::change;
use crate::{AppError, AppResult, UnitOfWork, resolve};

/// Moves the dependency `old` (from, to) to `new` (from, to) of kind `kind`. The cycle check and
/// the lock on cards that have started judge both ends; the old edge must exist.
pub fn move_edge<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    chain: &str,
    old: (&str, &str),
    new: (&str, &str),
    kind: EdgeKind,
) -> AppResult<ChainId> {
    change(uow, chain, |ctx| {
        let node = |r: &str| resolve::node(&ctx.view, r);
        let (of, ot) = (node(old.0)?, node(old.1)?);
        let (nf, nt): (NodeRef, NodeRef) = (node(new.0)?, node(new.1)?);
        let was = ctx
            .view
            .edges
            .iter()
            .find(|e| e.from == of && e.to == ot)
            .copied()
            .ok_or_else(|| AppError::not_found(format!("edge {} -> {}", old.0, old.1)))?;
        ctx.view.check_dependency_change(ot)?;
        ctx.view.check_dependency_change(nt)?;
        let edge = Edge {
            chain_id: ctx.view.chain.id,
            from: nf,
            to: nt,
            kind,
        };
        ctx.move_edge(was, edge)
    })
}

/// Moves a card that has not started to another epic of its project, in one change: the card
/// leaves its epic and joins `to_chain` at the end (after the cards nothing waits for yet) or
/// directly before or after `place`'s anchor. A cancelled card joins loose, with no place. The
/// key stays. Refusals (core's [`toknila_core::card_move`]) drop the whole move.
pub fn move_to_chain<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    ticket: &toknila_core::Ticket,
    to_chain: &str,
    place: Option<(&str, toknila_core::graph::Place)>,
) -> AppResult<()> {
    let from = uow
        .chain(ticket.chain_id)?
        .ok_or_else(|| AppError::not_found(format!("epic of {}", ticket.key)))?;
    let to = match resolve::chain_in(uow, Some(&from.project_id.to_string()), to_chain) {
        Err(e) if e.kind == crate::ErrorKind::NotFound => resolve::chain(uow, to_chain)?,
        other => other?,
    };
    let has_ship = uow.ship(ticket.id)?.is_some();
    if place.is_some() && ticket.status == toknila_core::TicketStatus::Cancelled {
        return Err(AppError::bad_request(format!(
            "{} is cancelled and has no place in an epic's order",
            ticket.key
        )));
    }
    let id = ticket.id;
    let card = crate::chain::run_chain(uow, from.id, |ctx| -> AppResult<_> {
        let linked = ctx.linked_cards(id);
        toknila_core::card_move::check(ticket, &from, &to, has_ship, &linked)
            .map_err(|e| AppError::conflict(e.to_string()))?;
        ctx.take_ticket(id)
    })?;
    let cancelled = card.status == toknila_core::TicketStatus::Cancelled;
    crate::with_chain(uow, to.id, |ctx| -> AppResult<()> {
        let tail: Vec<NodeRef> = ctx
            .view
            .tail_tickets()
            .into_iter()
            .map(NodeRef::Ticket)
            .collect();
        ctx.adopt_ticket(card, &from.name);
        if cancelled {
            return Ok(());
        }
        match place {
            Some((anchor, place)) => {
                let NodeRef::Ticket(anchor) = resolve::node(&ctx.view, anchor)? else {
                    return Err(AppError::bad_request("a story goes next to a story"));
                };
                let plan = ctx.view.plan_move(id, anchor, place)?;
                for e in plan.remove {
                    ctx.remove_edge(e.from, e.to);
                }
                for e in plan.add {
                    ctx.add_edge(e)?;
                }
            }
            None => {
                for from in tail {
                    ctx.add_edge(Edge {
                        chain_id: ctx.view.chain.id,
                        from,
                        to: NodeRef::Ticket(id),
                        kind: EdgeKind::Normal,
                    })?;
                }
            }
        }
        Ok(())
    })
}

/// Moves the design reference at `index` of `ticket` to the card `target` of the same chain.
pub fn move_design<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    ticket: &toknila_core::Ticket,
    index: usize,
    target: &str,
) -> AppResult<()> {
    let target = resolve::ticket(uow, target)?;
    if target.chain_id != ticket.chain_id {
        return Err(AppError::bad_request(format!(
            "{} is not in the epic of {}",
            target.key, ticket.key
        )));
    }
    crate::with_chain(uow, ticket.chain_id, |ctx| {
        ctx.move_design(ticket.id, index, target.id)
    })
}
