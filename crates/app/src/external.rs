//! Work that leaves the database: git, and later processes. A command never does it inside its
//! unit of work. It queues [`ExternalEffect`]s ([`crate::UnitOfWork::defer`]); the adapter commits
//! the command, then [`execute`]s them on the host, then runs [`apply`] in a second unit of work
//! so each outcome reaches the ticket as an event through the state machine.

use toknila_core::{Ship, ShipSpec, TicketId};

use crate::{AppResult, ShipError, Shipyard, UnitOfWork, ships};

/// Something to do on the host once the command that asked for it has committed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExternalEffect {
    /// Make (or reuse) the ticket's ship.
    PrepareShip { ticket: TicketId, spec: ShipSpec },
    /// Remove the ship's worktree from the repository at `repo`; the branch stays.
    RemoveShip { repo: String, ship: Ship },
}

/// What happened on the host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExternalOutcome {
    ShipReady { ticket: TicketId, ship: Ship },
    ShipFailed { ticket: TicketId, error: ShipError },
    ShipRemoved { ship: Ship },
    ShipRemoveFailed { ship: Ship, error: ShipError },
}

/// Runs the effects on the host, in order. Touches no database.
pub fn execute(yard: &dyn Shipyard, effects: Vec<ExternalEffect>) -> Vec<ExternalOutcome> {
    effects
        .into_iter()
        .map(|effect| match effect {
            ExternalEffect::PrepareShip { ticket, spec } => match yard.prepare(&spec) {
                Ok(path) => ExternalOutcome::ShipReady {
                    ticket,
                    ship: ships::ship_of(ticket, &spec, path),
                },
                Err(error) => ExternalOutcome::ShipFailed { ticket, error },
            },
            ExternalEffect::RemoveShip { repo, ship } => match yard.remove(&repo, &ship) {
                Ok(()) => ExternalOutcome::ShipRemoved { ship },
                Err(error) => ExternalOutcome::ShipRemoveFailed { ship, error },
            },
        })
        .collect()
}

/// Records the outcomes: state and journal, with ticket changes going through the machine.
pub fn apply<U: UnitOfWork + ?Sized>(uow: &mut U, outcomes: Vec<ExternalOutcome>) -> AppResult<()> {
    for outcome in outcomes {
        match outcome {
            ExternalOutcome::ShipReady { ticket, ship } => ships::ready(uow, ticket, ship)?,
            ExternalOutcome::ShipFailed { ticket, error } => ships::failed(uow, ticket, &error)?,
            ExternalOutcome::ShipRemoved { ship } => ships::removed(uow, ship)?,
            ExternalOutcome::ShipRemoveFailed { ship, error } => {
                ships::remove_failed(uow, ship, &error)?;
            }
        }
    }
    Ok(())
}
