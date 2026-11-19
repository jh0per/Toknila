//! The two ways a merge that is running ends inside the machine: it landed, or the base
//! conflicts with the ship and the card goes back to the worker.

use super::review::{Outcome, leave};
use super::{Effect, TransitionError};
use crate::{Actor, ReviewTurn, Ticket, TicketStatus, Timestamp, Verdict};

/// The merge landed: what that does to the host and the graph. `None` when no merge was running.
pub(super) fn done(next: &mut Ticket, effects: &mut Vec<Effect>) -> Option<Outcome> {
    if next.review.turn != Some(ReviewTurn::Merging) {
        return None;
    }
    next.review.turn = None;
    next.review.note = None;
    next.review.accepted = false;
    next.review.accept_withdrawn = None;
    effects.push(Effect::RetireShip);
    effects.push(Effect::StopRunner);
    effects.push(Effect::DependencySatisfied);
    Some((TicketStatus::Merged, None))
}

/// The reviewer just asked for changes (its round is the last): a person's remembered Accept
/// stands across the daemon's own rounds only, so new work voids it and the person looks again.
pub(super) fn withdraw_accept(next: &mut Ticket, effects: &mut Vec<Effect>) {
    if !next.review.accepted {
        return;
    }
    next.review.accepted = false;
    next.review.accept_withdrawn = Some(next.review.rounds.len().try_into().unwrap_or(u32::MAX));
    effects.push(Effect::RecordNotice {
        summary: crate::ACCEPT_WITHDRAWN.to_owned(),
    });
}

/// The merge of an accepted card conflicted: the card goes back to the worker with `text`, a
/// round like a hand-in-time conflict, and the person's Accept is remembered so that the next
/// hand-in that passes the check and the reviewer merges without asking again.
pub(super) fn conflicted(
    next: &mut Ticket,
    text: &str,
    now: Timestamp,
    effects: &mut Vec<Effect>,
) -> Result<Outcome, TransitionError> {
    if next.review.turn != Some(ReviewTurn::Merging) {
        return Err(TransitionError::NotAllowed {
            event: "merge_conflicted",
            from: next.status,
        });
    }
    leave(next, effects);
    next.review
        .record(Actor::System, Verdict::RequestChanges, text, now);
    next.review.accepted = true;
    effects.push(Effect::RecordCheckOutput {
        text: text.to_owned(),
    });
    Ok(super::goes_on::worker(next, crate::GoOn::SentBack, now))
}
