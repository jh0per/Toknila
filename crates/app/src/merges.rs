//! Merging an accepted card's ship branch into the project's base branch. Core decides when
//! (the `Merge` effect, from Accept); a [`Merger`] port does it on the host after the command
//! committed ([`crate::external`]); this module queues that work and turns the outcome into an
//! event for the ticket: `MergeDone`, or a block with a typed reason.

use toknila_core::guard::ShipProblem;
use toknila_core::merge_line::waits_for_github;
use toknila_core::pr::PrSpec;
use toknila_core::{
    Chain, MergeMode, MergeSpec, PullRequest, Recheck, ReviewTurn, ShipSpec, Ticket, TicketKey,
    TicketStatus,
};

use crate::external::{ExternalEffect, Host};
use crate::ships::{self, GuardError};
use crate::sync::{self, Seen, SyncOutcome, SyncSpec};
use crate::{AppResult, GitError, UnitOfWork};

mod line;
mod outcome;
pub use line::{advance, line_of, place_unplaced, take_release_turn};
pub(crate) use outcome::finished;

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
    /// Bringing the ship up to the base failed to merge; `detail` names the files and migrations.
    BaseConflict { detail: String },
    /// The base branch is checked out at `path` with local changes; nothing was changed.
    BaseCheckoutDirty { path: String },
    /// Pull-request mode: the pull request is open and this step did not merge it: its checks
    /// have not all finished, or they passed and the step had no merge turn (or pushed new work).
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
    /// After the sync main is at a commit the last passed check did not see: nothing was merged;
    /// the card goes back through the check. `commits` is how many main took since, when known.
    MainMoved { commits: Option<u32> },
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

    /// Merges the base into the ship's worktree when the base moved since the ship started (a
    /// merge commit, no rebase). A conflict, or a migration number both branches took, leaves the
    /// ship untouched and is [`SyncOutcome::Conflict`].
    fn sync(&self, _spec: &SyncSpec) -> Result<SyncOutcome, MergeError> {
        Err(MergeError::Io(
            "this merger cannot bring a ship up to its base".to_owned(),
        ))
    }

    /// The commit of main the synced ship sees and the ship's HEAD; with `since`, how many
    /// commits main has beyond it. `None` when the host cannot say.
    fn seen(&self, _spec: &SyncSpec, _since: Option<&str>) -> Option<Seen> {
        None
    }
}

/// Runs one step of a merge on the host. When the step would push or merge the ship's work (a
/// local merge, or opening the pull request) the work is looked at first: changes that are not
/// committed, or a branch with nothing of its own, end the merge as a typed outcome and nothing
/// is pushed. Watching a pull request that is already open does not look again. A step that
/// holds the merge turn brings the ship up to the base first (and checks it again when main
/// moved since its check), also for a pull request that is open: the base may have moved while
/// GitHub ran its checks. A step without the turn only looks at the pull request.
pub(crate) fn run(host: &Host<'_>, spec: &MergeSpec) -> Result<MergeOutcome, MergeError> {
    if spec.pr.as_ref().is_some_and(|pr| !pr.holds_turn) {
        return host.merger.merge(spec);
    }
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
    // Accept on a ship that fell behind: the base goes into the ship first.
    match host.merger.sync(&SyncSpec::of(spec))? {
        SyncOutcome::Conflict { files, migrations } => {
            return Ok(MergeOutcome::BaseConflict {
                detail: sync::conflict_text(&spec.look.start, &files, &migrations),
            });
        }
        SyncOutcome::UpToDate | SyncOutcome::Merged { .. } => {}
    }
    if let Some(moved) = main_moved(host, spec) {
        return Ok(moved);
    }
    host.merger.merge(spec)
}

/// Whether main, as the ship now sees it, is a commit the last passed check did not see (and the
/// project has a check command to run again).
fn main_moved(host: &Host<'_>, spec: &MergeSpec) -> Option<MergeOutcome> {
    let recheck = spec.recheck.as_ref()?;
    let Seen { base, commits, .. } = host
        .merger
        .seen(&SyncSpec::of(spec), recheck.base.as_deref())?;
    recheck
        .due(&base)
        .then_some(MergeOutcome::MainMoved { commits })
}

/// `Merge`: queue the merge for after the commit, when the card holds its project's merge line;
/// one that waits in it is queued when its turn comes ([`advance`]). A card without a ship (its
/// project has no repository, or it never started) has nothing to merge: the host answers right
/// away. In pull-request mode the spec says whether the pull request is already open (the card
/// has one) and whether the step holds the turn: a card whose pull request waits for GitHub's
/// checks is out of the line, and its step only looks. Returns whether a step was queued.
pub(crate) fn queue_merge<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    chain: &Chain,
    ticket: &Ticket,
) -> AppResult<bool> {
    let holds = ticket.review.merge_line.is_some_and(|p| p.holds);
    if !holds && !waits_for_github(ticket) {
        return Ok(false);
    }
    let project = uow
        .project(chain.project_id)?
        .ok_or_else(|| crate::AppError::not_found(format!("project of epic {}", chain.name)))?;
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
                merge.pr = Some(PrSpec::for_ticket(&project, ticket, holds));
            }
            // What the last check saw of main: Accept checks again when main is elsewhere now.
            merge.recheck = project
                .check_command
                .is_some()
                .then(|| Recheck::of(ticket.review.last_check.as_ref()));
            Some(merge)
        }
        _ => None,
    };
    uow.defer(ExternalEffect::Merge {
        ticket: ticket.id,
        spec,
    });
    Ok(true)
}

/// The daemon's ticker: every card whose pull request is open and whose merge is running gets
/// its pull request looked at again, after the commit: one that waits for GitHub only looks, the
/// one that holds the merge turn merges it. A card whose checks passed and that waits in the line
/// for its turn is not asked. Returns the keys asked.
pub fn poll<U: UnitOfWork + ?Sized>(uow: &mut U) -> AppResult<Vec<TicketKey>> {
    let mut asked = Vec::new();
    for chain in uow.chains(None)? {
        for t in uow.tickets(Some(chain.id))? {
            if t.status == TicketStatus::Review
                && t.review.turn == Some(ReviewTurn::Merging)
                && t.pr.is_some()
                && queue_merge(uow, &chain, &t)?
            {
                asked.push(t.key);
            }
        }
    }
    Ok(asked)
}

/// After a daemon start: a merge that was running when it stopped (its card holds the merge
/// line's turn) is queued again. It is safe to repeat: a merge that had landed finds its branch
/// already in the base. The cards that wait in the line keep waiting.
pub fn resume<U: UnitOfWork + ?Sized>(uow: &mut U) -> AppResult<Vec<TicketKey>> {
    let mut resumed = Vec::new();
    for chain in uow.chains(None)? {
        for t in uow.tickets(Some(chain.id))? {
            if t.status == TicketStatus::Review
                && t.review.turn == Some(ReviewTurn::Merging)
                && t.review.merge_line.is_some_and(|p| p.holds)
            {
                queue_merge(uow, &chain, &t)?;
                resumed.push(t.key);
            }
        }
    }
    Ok(resumed)
}
