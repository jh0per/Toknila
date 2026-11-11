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
            "{} is not on the chain of {}",
            target.key, ticket.key
        )));
    }
    crate::with_chain(uow, ticket.chain_id, |ctx| {
        ctx.move_design(ticket.id, index, target.id)
    })
}
