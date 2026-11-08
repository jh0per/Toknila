//! Merging an accepted card's ship branch into the project's base branch. Core decides when
//! (the `Merge` effect, from Accept); a [`Merger`] port does it on the host after the command
//! committed ([`crate::external`]); this module queues that work and turns the outcome into an
//! event for the ticket: `MergeDone`, or a block with a typed reason.

use toknila_core::guard::ShipProblem;
use toknila_core::machine::TicketEvent;
use toknila_core::pr::PrSpec;
use toknila_core::{
    BlockedReason, Chain, MergeMode, MergeSpec, PullRequest, ReviewTurn, ShipSpec, Ticket,
    TicketId, TicketKey, TicketStatus,
};

use crate::chain::tell_ticket;
use crate::external::{ExternalEffect, Host};
use crate::ships::{self, GuardError};
use crate::{AppResult, GitError, UnitOfWork};

/// How a merge ended when nothing went wrong on the host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeOutcome {
    /// The base branch now has a merge commit that contains the ship's branch.
    Merged { commit: String },
    /// The base branch already contains the ship's branch (or the card has no ship): nothing to do.
    UpToDate,
    /// The ship has changes that are not committed; nothing was pushed or merged.
    ShipUncommitted { files: Vec<String> },
    /// The ship's branch has no commit of its own; nothing was pushed or merged.
    ShipEmpty,
    /// The branches changed the same files; the base branch is untouched.
    Conflict { files: Vec<String> },
    /// The base branch is checked out at `path` with local changes; nothing was changed.
    BaseCheckoutDirty { path: String },
    /// Pull-request mode: the pull request is open and its checks have not all finished.
    PrOpen { pr: PullRequest },
    /// Pull-request mode: the checks finished and these failed.
    CiFailed {
        pr: PullRequest,
        failed: Vec<String>,
    },
    /// Pull-request mode: the pull request was closed on GitHub without merging.
    PrClosed { url: String },
    /// Pull-request mode: GitHub reports a merge conflict on the pull request.
    PrConflict { url: String },
    /// Pull-request mode: the pull request merged on GitHub and the base was fetched.
    PrMerged { pr: PullRequest },
}

/// Why a merge could not run or finish. The text lands in the ticket's blocked reason.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MergeError {
    /// A `git` command did not give a result.
    #[error(transparent)]
    Git(#[from] GitError),
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
    /// Pull-request mode needs the GitHub CLI.
    #[error(
        "gh (the GitHub CLI) is not installed or not on PATH; install it from https://cli.github.com, then unblock"
    )]
    GhMissing,
    /// `gh` has no GitHub login.
    #[error("gh is not logged in to GitHub; run `gh auth login`, then unblock")]
    GhNotLoggedIn,
    /// A `gh` command failed.
    #[error("gh {command} failed: {detail}")]
    Gh { command: String, detail: String },
    /// The file system refused.
    #[error("{0}")]
    Io(String),
}

/// What the application needs from the host to land a card's work. The daemon implements it
/// over `git`, and over `git` and `gh` when the spec has a pull request.
pub trait Merger: std::fmt::Debug {
    /// Merges `spec.branch` into `spec.base` of the repository at `spec.repo` with a merge commit.
    /// Safe to call again: a branch the base already contains is [`MergeOutcome::UpToDate`].
    ///
    /// With `spec.pr` the work goes through a pull request: the branch is pushed and the pull
    /// request opened or reused (`watching` is `None`), or only looked at (`watching` is set);
    /// the pull request is merged when every check passed. Each call is one step: while checks
    /// run the answer is [`MergeOutcome::PrOpen`] and the caller asks again later.
    fn merge(&self, spec: &MergeSpec) -> Result<MergeOutcome, MergeError>;
}

/// Runs one step of a merge on the host. When the step would push or merge the ship's work (a
/// local merge, or opening the pull request) the work is looked at first: changes that are not
/// committed, or a branch with nothing of its own, end the merge as a typed outcome and nothing
/// is pushed. Watching a pull request that is already open does not look again.
pub(crate) fn run(host: &Host<'_>, spec: &MergeSpec) -> Result<MergeOutcome, MergeError> {
    let watching = spec.pr.as_ref().is_some_and(|pr| pr.watching.is_some());
    if !watching {
        match ships::guard(host.yard, &spec.look) {
            Ok(()) => {}
            Err(GuardError::Problem(ShipProblem::Uncommitted { files })) => {
                return Ok(MergeOutcome::ShipUncommitted { files });
            }
            Err(GuardError::Problem(ShipProblem::Empty)) => return Ok(MergeOutcome::ShipEmpty),
            Err(GuardError::Ship(e)) => {
                return Err(MergeError::Io(format!("cannot look at the ship: {e}")));
            }
        }
    }
    host.merger.merge(spec)
}

/// `Merge`: queue the merge for after the commit. A card without a ship (its project has no
/// repository, or it never started) has nothing to merge: the host answers right away. In
/// pull-request mode the spec says whether the pull request is already open (the card has one):
/// then the host only looks at it.
pub(crate) fn queue_merge<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    chain: &Chain,
    ticket: &Ticket,
) -> AppResult<()> {
    let project = uow
        .project(chain.project_id)?
        .ok_or_else(|| crate::AppError::not_found(format!("project of chain {}", chain.name)))?;
    let spec = match (
        uow.ship(ticket.id)?,
        ShipSpec::for_ticket(&project, chain, ticket),
    ) {
        // The branch the ship was made with, not the one the title would name today.
        (Some(ship), Some(spec)) => {
            let mut merge = ShipSpec {
                branch: ship.branch.clone(),
                ..spec
            }
            .merge_spec(&ticket.title, &ship.path);
            if project.merge.mode == MergeMode::Github {
                merge.pr = Some(PrSpec::for_ticket(&project, ticket));
            }
            Some(merge)
        }
        _ => None,
    };
    uow.defer(ExternalEffect::Merge {
        ticket: ticket.id,
        spec,
    });
    Ok(())
}

/// The daemon's ticker: every card whose pull request is open and whose merge is running gets
/// its pull request looked at again, after the commit. Returns the keys asked.
pub fn poll<U: UnitOfWork + ?Sized>(uow: &mut U) -> AppResult<Vec<TicketKey>> {
    let mut asked = Vec::new();
    for chain in uow.chains(None)? {
        for t in uow.tickets(Some(chain.id))? {
            if t.status == TicketStatus::Review
                && t.review.turn == Some(ReviewTurn::Merging)
                && t.pr.is_some()
            {
                queue_merge(uow, &chain, &t)?;
                asked.push(t.key);
            }
        }
    }
    Ok(asked)
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
        Ok(MergeOutcome::PrMerged { pr }) => {
            record_pr(uow, ticket, pr)?;
            TicketEvent::MergeDone
        }
        Ok(MergeOutcome::PrOpen { pr }) => return record_pr(uow, ticket, pr),
        Ok(MergeOutcome::CiFailed { pr, failed }) => {
            let url = pr.url.clone();
            record_pr(uow, ticket, pr)?;
            block(BlockedReason::CiFailed {
                url,
                checks: failed,
            })
        }
        Ok(MergeOutcome::PrClosed { url }) => block(BlockedReason::PrClosed { url }),
        Ok(MergeOutcome::PrConflict { url }) => block(BlockedReason::MergeConflict {
            detail: format!(
                "GitHub reports a merge conflict on {url}. Resolve it on the ship (merge the base branch into it), unblock pushes it and watches the pull request again."
            ),
        }),
        Ok(MergeOutcome::Conflict { files }) => block(BlockedReason::MergeConflict {
            detail: format!(
                "conflicting files: {}. Resolve them on the ship (merge the base branch into it), then unblock.",
                files.join(", ")
            ),
        }),
        Ok(MergeOutcome::ShipUncommitted { files }) => {
            block(BlockedReason::ShipUncommitted { files })
        }
        Ok(MergeOutcome::ShipEmpty) => block(BlockedReason::ShipEmpty),
        Ok(MergeOutcome::BaseCheckoutDirty { path }) => {
            block(BlockedReason::BaseCheckoutDirty { path })
        }
        Err(e) => block(BlockedReason::MergeFailed {
            detail: e.to_string(),
        }),
    };
    tell_ticket(uow, ticket, event)
}

/// The card's pull request as the host last saw it (an unchanged one changes nothing, so the
/// journal gets no entry for it).
fn record_pr<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    ticket: TicketId,
    pr: PullRequest,
) -> AppResult<()> {
    tell_ticket(uow, ticket, TicketEvent::PrUpdated { pr })
}

fn block(reason: BlockedReason) -> TicketEvent {
    TicketEvent::Block { reason }
}
