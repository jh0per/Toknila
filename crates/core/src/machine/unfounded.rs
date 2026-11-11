//! A request for changes the reviewer could not found on anything it ran or read: no round, no
//! return to the worker, the card waits for a person with what the reviewer said.

use super::review::Outcome;
use super::{Effect, TransitionError};
use crate::{ReviewNote, ReviewTurn, Ticket, TicketStatus};

/// The most lines of the reviewer's reading a note keeps.
const NOTE_LINES: usize = 40;

/// The reviewer asked for changes without a failing item it ran or read: it was never able to
/// judge the work. The verdict is no round (`auto_returns` stays), the worker is not woken, and
/// the card waits for a person, who sees the reviewer's lines and can run the reviewer again.
pub(super) fn wait_for_person(
    next: &mut Ticket,
    text: &str,
    effects: &mut Vec<Effect>,
) -> Result<Outcome, TransitionError> {
    let lines: Vec<String> = text.lines().take(NOTE_LINES).map(str::to_owned).collect();
    next.review.turn = Some(ReviewTurn::Person);
    next.review.nudged = false;
    next.review.note = Some(ReviewNote::NoVerdict { lines });
    effects.push(Effect::StopReviewer);
    effects.push(Effect::RecordReport {
        summary: "The reviewer could not run what it needed, so its request for changes is not a finding against the code; it is for a person to decide.".to_owned(),
    });
    Ok((TicketStatus::Review, None))
}
