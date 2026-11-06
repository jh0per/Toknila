//! Ships: the worktree and branch a ticket works in. Core decides when one is needed (the
//! `PrepareShip` and `RemoveShip` effects) and what it is called ([`ShipSpec`]); a [`Shipyard`]
//! port makes and removes them; this module orders the two inside the chain unit of work.

use toknila_core::events::DomainEvent;
use toknila_core::machine::TicketEvent;
use toknila_core::{BlockedReason, Ship, ShipSpec, TicketId};

use crate::{AppError, AppResult, ChainTx};

/// Why a ship could not be made or removed. The text lands in the ticket's blocked reason.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ShipError {
    /// The `git` executable is not installed or not on the path.
    #[error("git is not installed or not on PATH")]
    GitMissing,
    /// The project's repository path is not a git repository.
    #[error("{0} is not a git repository")]
    NotARepository(String),
    /// The branch ships start from does not exist in the repository.
    #[error("base branch {0} does not exist in the repository")]
    BaseBranchMissing(String),
    /// The ship's path is taken by something that is not this ship's worktree.
    #[error("{0} exists but is not the worktree of this ship")]
    PathTaken(String),
    /// A git command failed.
    #[error("git {command} failed: {detail}")]
    Git { command: String, detail: String },
    /// The file system refused.
    #[error("{0}")]
    Io(String),
}

/// What the application needs from the host to give a ticket a ship. The daemon implements it
/// over `git`.
pub trait Shipyard: std::fmt::Debug {
    /// Makes the worktree and branch of `spec` and returns the worktree's absolute path. Safe to
    /// call again: an existing ship is reused, an existing branch is checked out as it is.
    fn prepare(&self, spec: &ShipSpec) -> Result<String, ShipError>;
    /// Removes the ship's worktree from the repository at `repo` and keeps its branch. A worktree
    /// that is already gone is fine.
    fn remove(&self, repo: &str, ship: &Ship) -> Result<(), ShipError>;
}

fn spec_for<U: crate::UnitOfWork + ?Sized>(
    uow: &U,
    ctx: &ChainTx,
    ticket: TicketId,
) -> AppResult<Option<(ShipSpec, toknila_core::TicketKey)>> {
    let Some(t) = ctx.view.tickets.iter().find(|t| t.id == ticket) else {
        return Ok(None);
    };
    let project = uow
        .project(ctx.view.chain.project_id)?
        .ok_or_else(|| AppError::not_found(format!("project of chain {}", ctx.view.chain.name)))?;
    Ok(ShipSpec::for_ticket(&project, &ctx.view.chain, t).map(|s| (s, t.key.clone())))
}

/// `PrepareShip`: make (or reuse) the ticket's ship and remember it. A failure blocks the ticket
/// through the state machine with `ship_failed`.
pub(crate) fn prepare<U: crate::UnitOfWork + ?Sized>(
    uow: &mut U,
    ctx: &mut ChainTx,
    ticket: TicketId,
) -> AppResult<()> {
    let Some((spec, key)) = spec_for(uow, ctx, ticket)? else {
        return Ok(());
    };
    let Some(yard) = uow.shipyard() else {
        return Ok(());
    };
    match yard.prepare(&spec) {
        Ok(path) => {
            let ship = Ship {
                ticket_id: ticket,
                key,
                branch: spec.branch,
                path,
            };
            if uow.ship(ticket)?.as_ref() == Some(&ship) {
                return Ok(());
            }
            uow.save_ship(&ship)?;
            uow.emit(&DomainEvent::ShipCreated { ship: ship.clone() })?;
            ctx.edit_ticket(ticket, |t| t.branch = Some(ship.branch.clone()))?;
        }
        Err(e) => ctx.ticket_event(
            ticket,
            TicketEvent::Block {
                reason: BlockedReason::ShipFailed {
                    detail: e.to_string(),
                },
            },
        )?,
    }
    Ok(())
}

/// `RemoveShip`: take the worktree away, keep the branch, forget the path. A ticket without a
/// ship is left alone; a worktree that cannot be removed fails the command.
pub(crate) fn remove<U: crate::UnitOfWork + ?Sized>(
    uow: &mut U,
    ctx: &ChainTx,
    ticket: TicketId,
) -> AppResult<()> {
    let Some(ship) = uow.ship(ticket)? else {
        return Ok(());
    };
    let Some((spec, _)) = spec_for(uow, ctx, ticket)? else {
        return Ok(());
    };
    let Some(yard) = uow.shipyard() else {
        return Ok(());
    };
    yard.remove(&spec.repo, &ship)
        .map_err(|e| AppError::conflict(format!("cannot remove the ship of {}: {e}", ship.key)))?;
    uow.delete_ship(ticket)?;
    uow.emit(&DomainEvent::ShipRemoved { ship })?;
    Ok(())
}
