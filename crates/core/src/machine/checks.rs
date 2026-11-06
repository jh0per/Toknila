//! Small guards the transition table shares.

use super::{Effect, TransitionError};
use crate::{Actor, BlockedReason, StatusReason, Ticket, TicketStatus};

/// Wildcard decisions belong to a person; an agent or the system cannot make them.
pub fn person_only(by: Actor, event: &'static str) -> Result<(), TransitionError> {
    if by == Actor::Human {
        Ok(())
    } else {
        Err(TransitionError::PersonOnly { event, actor: by })
    }
}

pub(super) fn non_empty(text: &str, what: &'static str) -> Result<(), TransitionError> {
    if text.trim().is_empty() {
        Err(TransitionError::EmptyText(what))
    } else {
        Ok(())
    }
}

pub(super) fn block(
    reason: &BlockedReason,
    ticket: &Ticket,
    effects: &mut Vec<Effect>,
) -> Result<(TicketStatus, Option<StatusReason>), TransitionError> {
    if let BlockedReason::Manual { text } = reason {
        non_empty(text, "block")?;
    }
    if matches!(
        reason,
        BlockedReason::Crashed { .. }
            | BlockedReason::RunnerLost
            | BlockedReason::ShipFailed { .. }
    ) {
        effects.push(Effect::StopRunner);
    }
    Ok((
        TicketStatus::Blocked,
        Some(StatusReason::Blocked {
            reason: reason.clone(),
            resume_to: ticket.status,
            waiting: match &ticket.status_reason {
                Some(StatusReason::Waiting(w)) => Some(w.clone()),
                _ => None,
            },
        }),
    ))
}

/// The ticket works on its ship: make sure it exists, then (re)start the runner there.
pub(super) fn run_on_ship(effects: &mut Vec<Effect>) {
    effects.push(Effect::PrepareShip);
    effects.push(Effect::EnsureRunner);
}
