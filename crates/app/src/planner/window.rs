//! What the planner tells the window: to show a card, and that its turn started or ended.

use toknila_core::events::DomainEvent;
use toknila_core::{ProjectId, Ticket};

use crate::access::Caller;
use crate::{AppError, AppResult, UnitOfWork};

/// Asks the window to show a card of the planner's project: one `window.show_card` event, no
/// change to the plan. Whether a window follows is for the window to decide.
pub fn show_card<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    project: ProjectId,
    reference: &str,
) -> AppResult<Ticket> {
    crate::as_caller::show_story(uow, &Caller::Planner(project), reference)
}

/// The planner's turn started (`working`) or ended: one `planner.turn` event, so every open
/// window learns of it from the stream. The turn itself and its steps live in the daemon's
/// memory (`toknila_core::planner::turn`); nothing else is stored.
pub fn turn<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    project: ProjectId,
    working: bool,
) -> AppResult<()> {
    uow.project(project)?
        .ok_or_else(|| AppError::not_found(format!("project {project}")))?;
    uow.emit(&DomainEvent::PlannerTurn { project, working })?;
    Ok(())
}
