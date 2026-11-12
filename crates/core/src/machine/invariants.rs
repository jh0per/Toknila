//! The rules every ticket must satisfy, whatever event led to it.

use crate::{StatusReason, Ticket, TicketStatus};

/// Invariants every ticket must satisfy. Returns the first violation.
pub fn check_invariants(ticket: &Ticket) -> Result<(), String> {
    use TicketStatus as S;
    match (ticket.status, &ticket.status_reason) {
        (S::Waiting, Some(StatusReason::Waiting(_))) => {}
        (
            S::Blocked,
            Some(StatusReason::Blocked {
                resume_to, waiting, ..
            }),
        ) => {
            if !matches!(resume_to, S::Running | S::Waiting | S::Review) {
                return Err(format!("blocked ticket resumes to {resume_to:?}"));
            }
            if waiting.is_some() != (*resume_to == S::Waiting) {
                return Err(format!(
                    "blocked ticket resumes to {resume_to:?} with {waiting:?}"
                ));
            }
        }
        (S::Cancelled, Some(StatusReason::Cancelled { .. })) => {}
        (S::Proposed, Some(StatusReason::Proposed)) => {}
        (S::Waiting | S::Blocked | S::Cancelled | S::Proposed, r) => {
            return Err(format!("{:?} with reason {r:?}", ticket.status));
        }
        (_, None) => {}
        (s, Some(r)) => return Err(format!("{s:?} must not carry a reason, has {r:?}")),
    }
    if ticket.review.turn.is_some() && ticket.status != S::Review {
        return Err(format!("awaiting a verdict in {:?}", ticket.status));
    }
    if ticket.review.check.is_some() && ticket.review.turn != Some(crate::ReviewTurn::Check) {
        return Err("a check phase outside the check turn".to_owned());
    }
    if ticket.paused && ticket.status != S::Backlog {
        return Err(format!("paused ticket in {:?}", ticket.status));
    }
    if ticket.agent_paused.is_some() && ticket.status != S::Waiting {
        return Err(format!("paused agent in {:?}", ticket.status));
    }
    if ticket.queued && ticket.status != S::Ready {
        return Err(format!("queued ticket in {:?}", ticket.status));
    }
    if ticket.status == S::Proposed && ticket.wildcard.as_ref().is_none_or(|w| w.approved) {
        return Err(format!("proposed ticket with mark {:?}", ticket.wildcard));
    }
    Ok(())
}
