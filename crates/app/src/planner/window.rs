//! What the planner asks of the window itself: to show a card.

use toknila_core::events::DomainEvent;
use toknila_core::{ProjectId, Ticket};

use super::card;
use crate::{AppError, AppResult, UnitOfWork};

/// Asks the window to show a card of the planner's project: one `window.show_card` event, no
/// change to the plan. Whether a window follows is for the window to decide.
pub fn show_card<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    project: ProjectId,
    reference: &str,
) -> AppResult<Ticket> {
    let t = card(&*uow, project, reference)?;
    let chain = uow
        .chain(t.chain_id)?
        .ok_or_else(|| AppError::not_found(format!("epic of {}", t.key)))?;
    let p = uow
        .project(project)?
        .ok_or_else(|| AppError::not_found(format!("project {project}")))?;
    uow.emit(&DomainEvent::WindowShowCard {
        project,
        project_key: p.key,
        chain: chain.name,
        ticket: t.key.clone(),
    })?;
    Ok(t)
}
