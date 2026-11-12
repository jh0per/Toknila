//! Small guards the transition table shares.

use super::{Effect, TicketEvent, TransitionError};
use crate::{Actor, BlockedReason, StalePolicy, StatusReason, Ticket, TicketStatus, WaitingReason};

/// Wildcard decisions belong to a person; an agent or the system cannot make them.
pub fn person_only(by: Actor, event: &'static str) -> Result<(), TransitionError> {
    if by == Actor::Human {
        Ok(())
    } else {
        Err(TransitionError::PersonOnly { event, actor: by })
    }
}

/// The reason of a card that is already Waiting when the agent asks again: a question replaces
/// the silent reason (a person must see what is asked), and silence never replaces a question.
/// Another question over an open one is refused.
pub(super) fn ask_again(
    ticket: &Ticket,
    asked: &WaitingReason,
    event: &TicketEvent,
) -> Result<(TicketStatus, Option<StatusReason>), TransitionError> {
    let reason = match &ticket.status_reason {
        Some(StatusReason::Waiting(WaitingReason::Silent)) => StatusReason::Waiting(asked.clone()),
        Some(open @ StatusReason::Waiting(_)) if *asked == WaitingReason::Silent => open.clone(),
        _ => {
            let (event, from) = (event.name(), ticket.status);
            return Err(TransitionError::NotAllowed { event, from });
        }
    };
    Ok((TicketStatus::Waiting, Some(reason)))
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
            | BlockedReason::Terminated { .. }
            | BlockedReason::RunnerLost
            | BlockedReason::ShipFailed { .. }
            | BlockedReason::SetupFailed { .. }
            | BlockedReason::BaseBehind { .. }
            | BlockedReason::NoWorkingDir
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

/// The policy a person's start decision gives, when the ticket is blocked because its base branch
/// is behind and the decision is possible; `None` means the event does not apply.
pub(super) fn stale_start(event: &TicketEvent, ticket: &Ticket) -> Option<StalePolicy> {
    let Some(StatusReason::Blocked {
        reason: BlockedReason::BaseBehind {
            can_fast_forward, ..
        },
        ..
    }) = &ticket.status_reason
    else {
        return None;
    };
    match event {
        TicketEvent::StartAnyway => Some(StalePolicy::Allow),
        TicketEvent::FastForwardStart if *can_fast_forward => Some(StalePolicy::FastForward),
        _ => None,
    }
}

/// [`run_on_ship`] after a person decided about a base branch that is behind.
pub(super) fn run_on_ship_with(effects: &mut Vec<Effect>, stale: StalePolicy) {
    effects.push(Effect::PrepareShipWith { stale });
    effects.push(Effect::EnsureRunner);
}

/// The ticket works on its ship: make sure it exists, then (re)start the runner there.
pub(super) fn run_on_ship(effects: &mut Vec<Effect>) {
    effects.push(Effect::PrepareShip);
    effects.push(Effect::EnsureRunner);
}
