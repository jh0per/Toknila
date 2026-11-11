//! The check turn of a card in Review: entering it, the phase the checker reports while it works
//! (syncing with main, running the command), and the ways the check ends without a verdict
//! (it could not run, or its program was not found).

use super::review::Outcome;
use super::{Effect, TransitionError};
use crate::{
    Actor, CheckPhase, ReviewMode, ReviewNote, ReviewTurn, Ticket, TicketStatus, Timestamp, Verdict,
};

/// The daemon starts checking the hand-in: the card waits for the check command's result.
pub(super) fn enter_check(next: &mut Ticket, effects: &mut Vec<Effect>) {
    effects.push(Effect::RunCheck);
    next.review.turn = Some(ReviewTurn::Check);
    next.review.check = None;
    next.review.nudged = false;
    next.review.note = None;
}

/// The checker reports where the check is. A card that is not in the check turn takes no phase.
pub(super) fn progress(
    next: &mut Ticket,
    event: &'static str,
    phase: CheckPhase,
) -> Result<Outcome, TransitionError> {
    if next.review.turn != Some(ReviewTurn::Check) {
        return Err(TransitionError::NotAllowed {
            event,
            from: next.status,
        });
    }
    next.review.check = Some(phase);
    Ok((TicketStatus::Review, None))
}

/// The round's comment when the project has no check command.
pub const NO_CHECK_COMMENT: &str = "Brought up to main; the project has no check command.";

/// The project has no check command and the ship was brought up to its base: a round that says
/// so (not "the check passed"), then on to whoever the review mode says.
pub(super) fn skipped(
    next: &mut Ticket,
    mode: ReviewMode,
    now: Timestamp,
    effects: &mut Vec<Effect>,
) -> Result<Outcome, TransitionError> {
    if next.review.turn != Some(ReviewTurn::Check) {
        return Err(TransitionError::NotAllowed {
            event: "check_skipped",
            from: next.status,
        });
    }
    next.review.check = None;
    next.review
        .record(Actor::System, Verdict::Approve, NO_CHECK_COMMENT, now);
    super::review::enter(next, mode, effects);
    Ok((TicketStatus::Review, None))
}

/// The phase "running the check command", started at `now`.
pub(super) fn running(command: &str, now: Timestamp) -> CheckPhase {
    CheckPhase::Running {
        command: command.to_owned(),
        since: now,
    }
}

/// The check could not run: the card waits for a person.
pub(super) fn check_lost(
    next: &mut Ticket,
    detail: &str,
    effects: &mut Vec<Effect>,
) -> Result<Outcome, TransitionError> {
    if next.review.turn != Some(ReviewTurn::Check) {
        return Err(TransitionError::NotAllowed {
            event: "check_lost",
            from: next.status,
        });
    }
    next.review.turn = Some(ReviewTurn::Person);
    next.review.check = None;
    effects.push(Effect::RecordNotice {
        summary: format!("The check could not run ({detail}); it is for a person to decide."),
    });
    Ok((TicketStatus::Review, None))
}

/// The check command's program was not found on this machine: the worker's work was never
/// judged, so no round is recorded and nothing is sent back. The card waits for a person, with a
/// note and a report that say what to fix on the machine.
pub(super) fn check_cannot_run(
    next: &mut Ticket,
    command: &str,
    effects: &mut Vec<Effect>,
) -> Result<Outcome, TransitionError> {
    if next.review.turn != Some(ReviewTurn::Check) {
        return Err(TransitionError::NotAllowed {
            event: "check_cannot_run",
            from: next.status,
        });
    }
    next.review.turn = Some(ReviewTurn::Person);
    next.review.check = None;
    next.review.note = Some(ReviewNote::CheckCannotRun {
        command: command.to_owned(),
    });
    effects.push(Effect::RecordNotice {
        summary: crate::toolpath::cannot_run_text(command),
    });
    Ok((TicketStatus::Review, None))
}
