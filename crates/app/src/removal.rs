//! Deleting a project. The records and their journal entries go in one command; what lies on disk
//! (the ships' worktrees, the run logs, the repository folder) is the host's to clean up after the
//! commit, from what [`delete_project`] returns. The repository's `ship/*` branches stay.

use toknila_core::events::DomainEvent;
use toknila_core::{Chain, Project, Ship, Ticket};

use crate::{AppResult, UnitOfWork, resolve};

/// What a deletion removed, for the host's cleanup after the commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Removed {
    /// The project as it was.
    pub project: Project,
    pub chains: Vec<Chain>,
    /// Every card of the project; their run logs go too.
    pub tickets: Vec<Ticket>,
    /// The ships the cards still had; their worktrees go too.
    pub ships: Vec<Ship>,
}

/// Deletes a project with its chains, cards, milestones and everything attached to them, and
/// their journal entries. The journal then tells of the deletion: one `ChainDeleted` per chain
/// and one `ProjectDeleted`. Refused while any agent of the project works.
pub fn delete_project<U: UnitOfWork + ?Sized>(uow: &mut U, project: &str) -> AppResult<Removed> {
    let p = resolve::project(uow, project)?;
    crate::projects::ensure_idle(uow, &p, "deleting")?;
    let chains = uow.chains(Some(p.id))?;
    let (mut tickets, mut ships) = (Vec::new(), Vec::new());
    for c in &chains {
        for t in uow.tickets(Some(c.id))? {
            ships.extend(uow.ship(t.id)?);
            tickets.push(t);
        }
    }
    uow.delete_project(p.id)?;
    for chain in &chains {
        uow.emit(&DomainEvent::ChainDeleted {
            chain: chain.clone(),
        })?;
    }
    uow.emit(&DomainEvent::ProjectDeleted { project: p.clone() })?;
    Ok(Removed {
        project: p,
        chains,
        tickets,
        ships,
    })
}
