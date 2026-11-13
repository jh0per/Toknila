//! The events of a card whose agent has no process: pausing it, wanting it back, losing it with
//! the app, and starting it again. The table is in [`super::apply`].

use super::checks::block;
use super::{Effect, TicketEvent, TransitionError};
use crate::{
    BlockedReason, PausedAgent, StatusReason, Ticket, TicketStatus, Timestamp, WaitingReason,
};

/// A runner lost again within this long after a resume that followed a loss is a loop: the card
/// goes to a person instead of resuming a second time.
const LOSS_LOOP_MS: i64 = 10 * 60 * 1000;

/// Whether `event` is one of the pause family a Running or Waiting card takes.
pub(super) fn is_pause(event: &TicketEvent) -> bool {
    matches!(
        event,
        TicketEvent::AgentPaused { .. }
            | TicketEvent::RunnerLost { .. }
            | TicketEvent::ResumeWanted
            | TicketEvent::ResumeStart
    )
}

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
                lost: false,
            });
            Some(stay)
        }
        (TicketEvent::RunnerLost { session }, _) => {
            lost(ticket, session.clone(), next, effects, now).ok()
        }
        (TicketEvent::ResumeWanted, Some(_)) => {
            next.agent_paused.as_mut()?.resume = true;
            Some(stay)
        }
        (TicketEvent::ResumeStart, Some(p)) => {
            if p.lost {
                next.lost_resume = Some(now);
            }
            effects.push(Effect::PrepareShip);
            effects.push(Effect::ResumeRunner {
                session: p.session.clone(),
                lost: p.lost,
            });
            Some((TicketStatus::Running, None))
        }
        _ => None,
    }
}

/// The runner of a Running or Waiting card is gone. A Running card wants its agent back at once; a
/// Waiting card keeps its question and waits for the answer. A second loss soon after a resume
/// that followed a loss blocks the card.
fn lost(
    ticket: &Ticket,
    session: Option<String>,
    next: &mut Ticket,
    effects: &mut Vec<Effect>,
    now: Timestamp,
) -> Result<(TicketStatus, Option<StatusReason>), TransitionError> {
    if let Some(first) = ticket.lost_resume
        && now.0.saturating_sub(first.0) < LOSS_LOOP_MS
    {
        next.lost_resume = None;
        let reason = BlockedReason::RunnerLost {
            first: Some(first),
            second: Some(now),
        };
        return block(&reason, ticket, effects);
    }
    next.agent_paused = Some(PausedAgent {
        session,
        since: now,
        resume: ticket.status == TicketStatus::Running,
        lost: true,
    });
    Ok((ticket.status, ticket.status_reason.clone()))
}

/// Whether a card keeps its paused agent after `event` took it to `status`: a Waiting card does,
/// and a Running one does only while its lost agent waits for a slot.
pub(super) fn keeps(ticket: &Ticket, status: TicketStatus, event: &TicketEvent) -> bool {
    status == TicketStatus::Waiting
        || (status == TicketStatus::Running
            && (matches!(
                event,
                TicketEvent::ResumeWanted | TicketEvent::RunnerLost { .. }
            ) || (*event == TicketEvent::Unblock && blocked_lost(ticket))))
}

/// Whether the card is Blocked because its runner was lost.
pub(super) fn blocked_lost(ticket: &Ticket) -> bool {
    matches!(
        &ticket.status_reason,
        Some(StatusReason::Blocked {
            reason: BlockedReason::RunnerLost { .. },
            resume_to: TicketStatus::Running | TicketStatus::Waiting,
            ..
        })
    )
}

/// Unblocking a card whose runner was lost: it never goes back to Running or Waiting with no
/// process. A Running card waits for its agent to be started again; a Waiting one keeps its
/// question and the answer starts the agent.
pub(super) fn unblock_lost(
    ticket: &Ticket,
    next: &mut Ticket,
    now: Timestamp,
) -> (TicketStatus, Option<StatusReason>) {
    let (resume_to, waiting) = match &ticket.status_reason {
        Some(StatusReason::Blocked {
            resume_to, waiting, ..
        }) => (*resume_to, waiting.clone()),
        _ => (TicketStatus::Running, None),
    };
    let running = resume_to != TicketStatus::Waiting;
    next.agent_paused = Some(PausedAgent {
        session: None,
        since: now,
        resume: running,
        lost: true,
    });
    if running {
        (TicketStatus::Running, None)
    } else {
        let w = waiting.unwrap_or(WaitingReason::Silent);
        (TicketStatus::Waiting, Some(StatusReason::Waiting(w)))
    }
}
