//! A story that has to go on with an agent never starts one itself. The transitions that would
//! (a send-back, a Return, a merge conflict, an Unblock, a start despite a stale base, a hand-in
//! that the reviewer agent reviews) only mark it: the worker's `agent_paused` with a
//! [`GoOn`](crate::GoOn), or `Review::reviewer_waits`. The schedule sends `ResumeStart` when one
//! of the project's agents is free, and that emits the start (see `crate::schedule`).

use super::review::Outcome;
use super::{Effect, TransitionError};
use crate::{GoOn, PausedAgent, ReviewTurn, StalePolicy, Ticket, TicketStatus, Timestamp};

/// The card goes on with its worker (`why`), whose new run starts when an agent is free.
pub(super) fn worker(next: &mut Ticket, why: GoOn, now: Timestamp) -> Outcome {
    next.agent_paused = Some(PausedAgent {
        session: None,
        since: now,
        resume: true,
        lost: false,
        go_on: Some(why),
    });
    (TicketStatus::Running, None)
}

/// The reviewer agent's turn came: it starts when an agent is free.
pub(super) fn reviewer(next: &mut Ticket) {
    next.review.reviewer_waits = true;
}

/// The schedule gave a card whose reviewer waits an agent: the reviewer starts. `None` when the
/// card's reviewer does not wait.
pub(super) fn reviewer_starts(next: &mut Ticket, effects: &mut Vec<Effect>) -> Option<Outcome> {
    if !next.review.reviewer_waits || !matches!(next.review.turn, Some(ReviewTurn::Agent { .. })) {
        return None;
    }
    next.review.reviewer_waits = false;
    effects.push(Effect::StartReviewer);
    Some((TicketStatus::Review, None))
}

/// What the start of a worker that has no process emits: a story that went on starts a new run
/// on its ship, as a start does; a paused or lost agent resumes its session.
pub(super) fn worker_starts(paused: &PausedAgent, effects: &mut Vec<Effect>) {
    match paused.go_on {
        Some(GoOn::Unblocked { stale }) if stale != StalePolicy::Refuse => {
            effects.push(Effect::PrepareShipWith { stale });
            effects.push(Effect::EnsureRunner);
        }
        Some(_) => {
            effects.push(Effect::PrepareShip);
            effects.push(Effect::EnsureRunner);
        }
        None => {
            effects.push(Effect::PrepareShip);
            effects.push(Effect::ResumeRunner {
                session: paused.session.clone(),
                lost: paused.lost,
            });
        }
    }
}

/// A round of the loop asked for changes: the card goes back to the worker with `text`, unless
/// `max_rounds` returns were already used since a person last decided; then it waits for the
/// person. `by_check` says whether the text is the check's (and so not the reviewer's remarks).
pub(super) fn send_back(
    next: &mut Ticket,
    (max_rounds, now): (u32, Timestamp),
    what: &str,
    text: &str,
    effects: &mut Vec<Effect>,
    by_check: bool,
) -> Result<Outcome, TransitionError> {
    let used = next.review.auto_returns();
    next.review.nudged = false;
    if used > max_rounds {
        next.review.turn = Some(ReviewTurn::Person);
        effects.push(Effect::RecordNotice {
            summary: format!(
                "{what} again, and the {max_rounds} rounds of the loop are used; it is for a person to decide."
            ),
        });
        return Ok((TicketStatus::Review, None));
    }
    next.review.turn = None;
    effects.push(if by_check {
        Effect::RecordCheckOutput {
            text: format!(
                "{what} (round {used} of {max_rounds}). Fix what it reports, then finish again.\n\n{text}"
            ),
        }
    } else {
        Effect::RecordRemarks {
            text: text.to_owned(),
        }
    });
    Ok(worker(next, GoOn::SentBack, now))
}
