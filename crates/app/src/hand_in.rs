//! What a ship carries beyond its hand-in, and what a hand-in after a Return changed since it.
//! Looked at on the host (never inside a unit of work) through the [`Shipyard`] port.

use toknila_core::guard::{self, ShipCommit};
use toknila_core::{Review, ShipLook, SinceReturn};

use crate::AppError;
use crate::ships::{ShipError, Shipyard};

/// The commits Accept refuses: those on the ship's own line after the hand-in that are neither
/// the hand-in nor a base-sync merge the daemon recorded.
pub fn extra_commits(
    yard: &dyn Shipyard,
    look: &ShipLook,
    review: &Review,
) -> Result<Vec<ShipCommit>, ShipError> {
    let Some(handed_in) = review.handed_in.as_deref() else {
        return Ok(Vec::new());
    };
    let after = yard.commits_after(look, handed_in)?;
    Ok(guard::extra_commits(&after, &review.synced))
}

/// The error Accept answers with when the ship carries commits beyond the hand-in.
pub fn refuse_extra(extra: &[ShipCommit], handed_in: &str) -> AppError {
    AppError::conflict(guard::extra_for_person(extra, handed_in))
}

/// The block for a hand-in at `head` that answers the Return of `review`: what changed from the
/// returned commit to `head`. `None` when no Return is open or git cannot say.
pub fn since_return(
    yard: &dyn Shipyard,
    look: &ShipLook,
    review: &Review,
    head: &str,
) -> Option<SinceReturn> {
    let round = review.open_return()?;
    let from = review.handed_in.as_deref()?;
    let (commits, files) = yard.changes_between(look, from, head).ok()?;
    Some(SinceReturn::new(round.number, from, head, commits, files))
}
