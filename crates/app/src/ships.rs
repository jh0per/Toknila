//! Ships: the worktree and branch a ticket works in. Core decides when one is needed (the
//! `PrepareShip` and `RemoveShip` effects) and what it is called ([`ShipSpec`]); a [`Shipyard`]
//! port makes and removes them after the command committed ([`crate::external`]); this module
//! queues that work and records what came of it.

use toknila_core::events::DomainEvent;
use toknila_core::machine::TicketEvent;
use toknila_core::{BlockedReason, Chain, Ship, ShipSpec, Ticket, TicketId};

use crate::external::ExternalEffect;

use crate::chain::tell_ticket;
use crate::{AppError, AppResult, ChainTx, UnitOfWork};

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

fn spec_for<U: UnitOfWork + ?Sized>(
    uow: &U,
    ctx: &ChainTx,
    ticket: TicketId,
) -> AppResult<Option<ShipSpec>> {
    match ctx.view.tickets.iter().find(|t| t.id == ticket) {
        Some(t) => spec_of(uow, &ctx.view.chain, t),
        None => Ok(None),
    }
}

/// The ship `ticket` of `chain` has or would have; `None` when its project has no repository.
pub(crate) fn spec_of<U: UnitOfWork + ?Sized>(
    uow: &U,
    chain: &Chain,
    ticket: &Ticket,
) -> AppResult<Option<ShipSpec>> {
    let project = uow
        .project(chain.project_id)?
        .ok_or_else(|| AppError::not_found(format!("project of chain {}", chain.name)))?;
    Ok(ShipSpec::for_ticket(&project, chain, ticket))
}

/// The ship a spec stands for once the shipyard gave it a path.
pub(crate) fn ship_of(ticket: TicketId, spec: &ShipSpec, path: String) -> Ship {
    Ship {
        ticket_id: ticket,
        key: spec.key.clone(),
        branch: spec.branch.clone(),
        path,
    }
}

/// `PrepareShip`: queue making (or reusing) the ticket's ship for after the commit.
pub(crate) fn queue_prepare<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    ctx: &ChainTx,
    ticket: TicketId,
) -> AppResult<()> {
    if let Some(spec) = spec_for(uow, ctx, ticket)? {
        uow.defer(ExternalEffect::PrepareShip { ticket, spec });
    }
    Ok(())
}

/// `RemoveShip`: queue taking the worktree away for after the commit; a ticket without a ship is
/// left alone.
pub(crate) fn queue_remove<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    ctx: &ChainTx,
    ticket: TicketId,
) -> AppResult<()> {
    let (Some(ship), Some(spec)) = (uow.ship(ticket)?, spec_for(uow, ctx, ticket)?) else {
        return Ok(());
    };
    uow.defer(ExternalEffect::RemoveShip {
        repo: spec.repo,
        ship,
    });
    Ok(())
}

/// The shipyard made the ship: remember it, journal it, tell the ticket its branch.
pub(crate) fn ready<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    ticket: TicketId,
    ship: Ship,
) -> AppResult<()> {
    if uow.ship(ticket)?.as_ref() == Some(&ship) {
        return Ok(());
    }
    uow.save_ship(&ship)?;
    uow.emit(&DomainEvent::ShipCreated { ship: ship.clone() })?;
    tell_ticket(
        uow,
        ticket,
        TicketEvent::ShipReady {
            branch: ship.branch,
        },
    )
}

/// The shipyard could not make the ship: the ticket is blocked with `ship_failed`.
pub(crate) fn failed<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    ticket: TicketId,
    error: &ShipError,
) -> AppResult<()> {
    let reason = BlockedReason::ShipFailed {
        detail: error.to_string(),
    };
    tell_ticket(uow, ticket, TicketEvent::Block { reason })
}

/// The worktree is gone: forget its path and journal it. The branch stays.
pub(crate) fn removed<U: UnitOfWork + ?Sized>(uow: &mut U, ship: Ship) -> AppResult<()> {
    uow.delete_ship(ship.ticket_id)?;
    uow.emit(&DomainEvent::ShipRemoved { ship })
}

/// The worktree could not be removed: the journal says so; the ticket and its accept stand.
pub(crate) fn remove_failed<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    ship: Ship,
    error: &ShipError,
) -> AppResult<()> {
    uow.emit(&DomainEvent::ShipRemoveFailed {
        ship,
        detail: error.to_string(),
    })
}
