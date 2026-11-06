//! Merging an accepted card's ship branch into the project's base branch. Core decides when
//! (the `Merge` effect, from Accept); a [`Merger`] port does it on the host after the command
//! committed ([`crate::external`]); this module queues that work and turns the outcome into an
//! event for the ticket: `MergeDone`, or a block with a typed reason.

use toknila_core::machine::TicketEvent;
use toknila_core::{
    BlockedReason, Chain, MergeSpec, ReviewTurn, ShipSpec, Ticket, TicketId, TicketKey,
    TicketStatus,
};

use crate::chain::tell_ticket;
use crate::external::ExternalEffect;
use crate::ships::spec_of;
use crate::{AppResult, UnitOfWork};

/// How a merge ended when nothing went wrong on the host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeOutcome {
    /// The base branch now has a merge commit that contains the ship's branch.
    Merged { commit: String },
    /// The base branch already contains the ship's branch (or the card has no ship): nothing to do.
    UpToDate,
    /// The branches changed the same files; the base branch is untouched.
    Conflict { files: Vec<String> },
    /// The base branch is checked out at `path` with local changes; nothing was changed.
    BaseCheckoutDirty { path: String },
}

/// Why a merge could not run or finish. The text lands in the ticket's blocked reason.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MergeError {
    /// The `git` executable is not installed or not on the path.
    #[error("git is not installed or not on PATH")]
    GitMissing,
    /// Merging without a working tree needs a newer `git`.
    #[error("git {found} is too old to merge without a working tree; {needs} or newer is needed")]
    GitTooOld { found: String, needs: String },
    /// The base branch does not exist in the repository.
    #[error("base branch {0} does not exist in the repository")]
    BaseMissing(String),
    /// The ship's branch does not exist in the repository.
    #[error("ship branch {0} does not exist in the repository")]
    BranchMissing(String),
    /// The base branch moved every time the merge tried to update it.
    #[error("base branch {0} kept moving while merging; unblock to try again")]
    BaseMoved(String),
    /// A git command failed.
    #[error("git {command} failed: {detail}")]
    Git { command: String, detail: String },
    /// The file system refused.
    #[error("{0}")]
    Io(String),
}

/// What the application needs from the host to land a card's work. The daemon implements it
/// over `git`; a pull-request mode plugs in here too.
pub trait Merger: std::fmt::Debug {
    /// Merges `spec.branch` into `spec.base` of the repository at `spec.repo` with a merge commit.
    /// Safe to call again: a branch the base already contains is [`MergeOutcome::UpToDate`].
    fn merge(&self, spec: &MergeSpec) -> Result<MergeOutcome, MergeError>;
}

/// `Merge`: queue the merge for after the commit. A card without a ship (its project has no
/// repository, or it never started) has nothing to merge: the host answers right away.
pub(crate) fn queue_merge<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    chain: &Chain,
    ticket: &Ticket,
) -> AppResult<()> {
    let spec = match (uow.ship(ticket.id)?, spec_of(uow, chain, ticket)?) {
        // The branch the ship was made with, not the one the title would name today.
        (Some(ship), Some(spec)) => Some(
            ShipSpec {
                branch: ship.branch,
                ..spec
            }
            .merge_spec(&ticket.title),
        ),
        _ => None,
    };
    uow.defer(ExternalEffect::Merge {
        ticket: ticket.id,
        spec,
    });
    Ok(())
}

/// After a daemon start: a merge that was running when it stopped is queued again. It is safe to
/// repeat: a merge that had landed finds its branch already in the base.
pub fn resume<U: UnitOfWork + ?Sized>(uow: &mut U) -> AppResult<Vec<TicketKey>> {
    let mut resumed = Vec::new();
    for chain in uow.chains(None)? {
        for t in uow.tickets(Some(chain.id))? {
            if t.status == TicketStatus::Review && t.review.turn == Some(ReviewTurn::Merging) {
                queue_merge(uow, &chain, &t)?;
                resumed.push(t.key);
            }
        }
    }
    Ok(resumed)
}

/// The host answered: the ticket is told through the machine.
pub(crate) fn finished<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    ticket: TicketId,
    result: Result<MergeOutcome, MergeError>,
) -> AppResult<()> {
    let event = match result {
        Ok(MergeOutcome::Merged { .. } | MergeOutcome::UpToDate) => TicketEvent::MergeDone,
        Ok(MergeOutcome::Conflict { files }) => block(BlockedReason::MergeConflict {
            detail: format!(
                "conflicting files: {}. Resolve them on the ship (merge the base branch into it), then unblock.",
                files.join(", ")
            ),
        }),
        Ok(MergeOutcome::BaseCheckoutDirty { path }) => {
            block(BlockedReason::BaseCheckoutDirty { path })
        }
        Err(e) => block(BlockedReason::MergeFailed {
            detail: e.to_string(),
        }),
    };
    tell_ticket(uow, ticket, event)
}

fn block(reason: BlockedReason) -> TicketEvent {
    TicketEvent::Block { reason }
}
