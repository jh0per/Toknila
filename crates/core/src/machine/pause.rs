//! The events of a Waiting card whose agent's process was ended: pausing it, wanting it back, and
//! starting it again. The table is in [`super::apply`].

use super::{Effect, TicketEvent};
use crate::{PausedAgent, StatusReason, Ticket, TicketStatus, Timestamp};

/// The status and reason after a pause event, or `None` when the card is not paused (or the event
/// is not one of these). `next` and `effects` are the transition being built.
pub(super) fn step(
    ticket: &Ticket,
    event: &TicketEvent,
    next: &mut Ticket,
    effects: &mut Vec<Effect>,
    now: Timestamp,
) -> Option<(TicketStatus, Option<StatusReason>)> {
    let stay = (ticket.status, ticket.status_reason.clone());
    match (event, ticket.agent_paused.as_ref()) {
        (TicketEvent::AgentPaused { session }, _) => {
            next.agent_paused = Some(PausedAgent {
                session: session.clone(),
                since: now,
                resume: false,
            });
            Some(stay)
        }
        (TicketEvent::ResumeWanted, Some(_)) => {
            next.agent_paused.as_mut()?.resume = true;
            Some(stay)
        }
        (TicketEvent::ResumeStart, Some(p)) => {
            effects.push(Effect::PrepareShip);
            effects.push(Effect::ResumeRunner {
                session: p.session.clone(),
            });
            Some((TicketStatus::Running, None))
        }
        _ => None,
    }
}
