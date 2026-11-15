//! Ships of finished stories: a merged or cancelled card keeps no workspace. Retiring is the one
//! path every merge takes (`ships::queue_retire`); this module finds the ships an older daemon (or
//! a restart between a commit and its host work) left behind, lists them, and retires them.

use toknila_core::{ProjectId, Ship, Ticket, TicketKey, TicketStatus};

use crate::external::ExternalEffect;
use crate::{AppResult, Reader, UnitOfWork, ships};

/// A ship record with the card that owns it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Workspace {
    pub ticket: Ticket,
    pub ship: Ship,
}

impl Workspace {
    /// Whether the card is done with: merged (or released) or cancelled.
    pub fn finished(&self) -> bool {
        is_finished(self.ticket.status)
    }
}

fn is_finished(status: TicketStatus) -> bool {
    matches!(
        status,
        TicketStatus::Merged | TicketStatus::Released | TicketStatus::Cancelled
    )
}

/// Every ship record of the cards of `project` (every project when `None`).
pub fn workspaces<R: Reader + ?Sized>(
    r: &R,
    project: Option<ProjectId>,
) -> AppResult<Vec<Workspace>> {
    let mut out = Vec::new();
    for chain in r.chains(project)? {
        for ticket in r.tickets(Some(chain.id))? {
            if let Some(ship) = r.ship(ticket.id)? {
                out.push(Workspace { ticket, ship });
            }
        }
    }
    Ok(out)
}

/// Queues the retirement of every ship of a finished card of `project` (every project when
/// `None`), for after the commit. Returns the workspaces queued, in card order.
pub fn retire_finished<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    project: Option<ProjectId>,
) -> AppResult<Vec<Workspace>> {
    let mut queued = Vec::new();
    for chain in uow.chains(project)? {
        for ticket in uow.tickets(Some(chain.id))? {
            if !is_finished(ticket.status) {
                continue;
            }
            let Some(ship) = uow.ship(ticket.id)? else {
                continue;
            };
            let Some(spec) = ships::spec_of(uow, &chain, &ticket)? else {
                continue;
            };
            uow.defer(ExternalEffect::RetireShip {
                start: spec.start_point(),
                repo: spec.repo,
                ship: ship.clone(),
            });
            queued.push(Workspace { ticket, ship });
        }
    }
    Ok(queued)
}

/// The keys of `queued`, for a log line or a report.
pub fn keys(queued: &[Workspace]) -> Vec<TicketKey> {
    queued.iter().map(|w| w.ticket.key.clone()).collect()
}
