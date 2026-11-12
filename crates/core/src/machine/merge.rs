//! The two ways a merge that is running ends inside the machine: it landed, or the base
//! conflicts with the ship and the card goes back to the worker.

use super::checks::run_on_ship;
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
    effects.push(Effect::RemoveShip);
    effects.push(Effect::StopRunner);
    effects.push(Effect::DependencySatisfied);
    Some((TicketStatus::Merged, None))
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
    run_on_ship(effects);
    Ok((TicketStatus::Running, None))
}
