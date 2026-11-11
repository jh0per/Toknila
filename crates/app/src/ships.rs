//! Ships: the worktree and branch a ticket works in. Core decides when one is needed (the
//! `PrepareShip` and `RemoveShip` effects) and what it is called ([`ShipSpec`]); a [`Shipyard`]
//! port makes and removes them after the command committed ([`crate::external`]); this module
//! queues that work and records what came of it.

use toknila_core::events::DomainEvent;
use toknila_core::guard::{self, ShipCommit, ShipProblem, ShipState};
use toknila_core::machine::TicketEvent;
use toknila_core::strays::Stray;
use toknila_core::{
    Actor, BaseBehind, BlockedReason, Chain, CommentBody, CommentKind, PlannerSite, PlannerSpec,
    Ship, ShipLook, ShipSpec, StalePolicy, Ticket, TicketId,
};

use crate::external::ExternalEffect;
use crate::notes::add_comment;

use crate::chain::tell_ticket;
use crate::{AppError, AppResult, ChainTx, GitError, Reader, UnitOfWork};

/// Why a ship could not be made or removed. The text lands in the ticket's blocked reason.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ShipError {
    /// A `git` command did not give a result.
    #[error(transparent)]
    Git(#[from] GitError),
    /// The project's repository path is not a git repository.
    #[error("{0} is not a git repository")]
    NotARepository(String),
    /// The branch ships start from does not exist in the repository.
    #[error("base branch {0} does not exist in the repository")]
    BaseBranchMissing(String),
    /// Local merge mode: the base branch is behind its upstream, so the ship would start on old
    /// code. The start is refused until a person decides.
    #[error("{0}")]
    BaseBehind(BaseBehind),
    /// The ship's path is taken by something that is not this ship's worktree.
    #[error("{0} exists but is not the worktree of this ship")]
    PathTaken(String),
    /// The file system refused.
    #[error("{0}")]
    Io(String),
}

/// What became of a cancelled ship's branch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchFate {
    /// It had no commits of its own and was deleted (or was already gone).
    Deleted,
    /// It has this many commits beyond its start and stays.
    Kept { ahead: u32 },
}

/// What the application needs from the host to give a ticket a ship. The daemon implements it
/// over `git`.
pub trait Shipyard: std::fmt::Debug {
    /// Makes the worktree and branch of `spec` and returns the worktree's absolute path. Safe to
    /// call again: an existing ship is reused, an existing branch is checked out as it is.
    fn prepare(&self, spec: &ShipSpec) -> Result<String, ShipError>;
    /// Removes the ship's worktree from the repository at `repo` and keeps its branch. A worktree
    /// that is already gone is fine.
    fn remove(&self, repo: &str, ship: &Ship) -> Result<(), ShipError>;
    /// Removes the ship's worktree like [`Self::remove`], then deletes its branch when it has no
    /// commits beyond `start`; a branch with commits stays. Says which.
    fn retire(&self, repo: &str, ship: &Ship, start: &str) -> Result<BranchFate, ShipError>;
    /// Every worktree under the yard's folder and every `ship/*` branch of the repository at
    /// `repo`, with the commits each branch has beyond `base`. A yard without any says none.
    fn found(&self, repo: &str, base: &str) -> Result<Vec<Stray>, ShipError> {
        let _ = (repo, base);
        Ok(Vec::new())
    }
    /// Removes one stray worktree (its checkout is discarded) or deletes one stray branch,
    /// whatever commits it has. Already gone is fine.
    fn discard(&self, repo: &str, stray: &Stray) -> Result<(), ShipError> {
        let _ = (repo, stray);
        Ok(())
    }
    /// Looks at the worktree at `look.path`: the changes in it that are not committed, and the
    /// commits its branch has beyond `look.start`. A yard that cannot look says the ship holds
    /// one clean commit, so nothing is refused for lack of an answer.
    fn inspect(&self, look: &ShipLook) -> Result<ShipState, ShipError> {
        let _ = look;
        Ok(ShipState {
            changed: Vec::new(),
            ahead: 1,
            head: None,
        })
    }
    /// The commits on the ship's own line (first parents) after `from`, newest first. A yard
    /// that cannot look says none.
    fn commits_after(&self, look: &ShipLook, from: &str) -> Result<Vec<ShipCommit>, ShipError> {
        let _ = (look, from);
        Ok(Vec::new())
    }
    /// The commit subjects (newest first) and the changed files from `from` to `to`. A yard that
    /// cannot look says none.
    fn changes_between(
        &self,
        look: &ShipLook,
        from: &str,
        to: &str,
    ) -> Result<(Vec<String>, Vec<String>), ShipError> {
        let _ = (look, from, to);
        Ok((Vec::new(), Vec::new()))
    }
    /// Gets the place the project's planner reads the code from, fresh as far as the network
    /// allows (a fetch that fails is fine): in pull-request mode a checkout of the fetched
    /// `origin/<base>` of its own, in local mode the working copy, with what is known about its
    /// base being behind. A yard that cannot do better gives the repository as it is.
    fn planner_site(&self, spec: &PlannerSpec) -> PlannerSite {
        PlannerSite {
            dir: spec.repo.clone(),
            behind: None,
        }
    }
    /// Removes the planner's checkout of `project` (the path-safe name) from the repository at
    /// `repo`; none is fine.
    fn remove_planner_site(&self, repo: &str, project: &str) -> Result<(), ShipError> {
        let _ = (repo, project);
        Ok(())
    }
}

fn spec_for<U: UnitOfWork + ?Sized>(
    uow: &U,
    ctx: &ChainTx,
    ticket: TicketId,
) -> AppResult<Option<ShipSpec>> {
    match ctx.view.tickets.iter().find(|t| t.id == ticket) {
        Some(t) => spec_of(uow, &ctx.view.chain, t),
        None => Ok(None),
    }
}

/// The ship `ticket` of `chain` has or would have; `None` when its project has no repository.
pub(crate) fn spec_of<U: Reader + ?Sized>(
    uow: &U,
    chain: &Chain,
    ticket: &Ticket,
) -> AppResult<Option<ShipSpec>> {
    let project = uow
        .project(chain.project_id)?
        .ok_or_else(|| AppError::not_found(format!("project of chain {}", chain.name)))?;
    Ok(ShipSpec::for_ticket(&project, chain, ticket))
}

/// Why the guard did not let a ship's work through.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuardError {
    /// The work is not in commits (core's verdict).
    Problem(ShipProblem),
    /// The yard could not look.
    Ship(ShipError),
}

/// Looks at a ship through the yard and judges it. Runs on the host, never inside a unit of work.
pub fn guard(yard: &dyn Shipyard, look: &ShipLook) -> Result<(), GuardError> {
    guarded(yard, look).map(|_| ())
}

/// [`guard`] that also hands back what it saw, for the hand-in to record the commit.
pub fn guarded(yard: &dyn Shipyard, look: &ShipLook) -> Result<ShipState, GuardError> {
    let state = yard.inspect(look).map_err(GuardError::Ship)?;
    guard::judge(&state).map_err(GuardError::Problem)?;
    Ok(state)
}

/// The error `finished` answers with when the card was returned and the ship is still at the
/// commit that was handed in: it quotes the Return, and the card stays Running.
pub fn refuse_unchanged(round: &toknila_core::ReviewRound, head: &str) -> AppError {
    AppError::conflict(guard::unchanged_for_agent(round, head))
}

/// Where to look at the ticket's ship when its agent says `finished`; `None` for a ticket
/// without a ship (sim tickets and projects without a repository), which has nothing to check.
pub fn finished_look<R: Reader + ?Sized>(r: &R, t: &Ticket) -> AppResult<Option<ShipLook>> {
    let (Some(ship), Some(chain)) = (r.ship(t.id)?, r.chain(t.chain_id)?) else {
        return Ok(None);
    };
    Ok(spec_of(r, &chain, t)?.map(|spec| ShipLook {
        path: ship.path,
        start: spec.start_point(),
    }))
}

/// The error `finished` answers with when the ship's work is not in commits: the agent's text
/// says what to do, and the card stays Running.
pub fn refuse_finished(error: GuardError, look: &ShipLook) -> AppError {
    match error {
        GuardError::Problem(p) => AppError::conflict(p.for_agent(&look.start)),
        GuardError::Ship(e) => AppError::conflict(format!(
            "Not finished: Toknila could not look at the ship to see that the work is committed ({e}). Try again."
        )),
    }
}

/// The error Accept answers with when the workspace differs from the hand-in commit: it names
/// the files, and the card stays in Review.
pub fn refuse_accept(error: GuardError, look: &ShipLook) -> AppError {
    match error {
        GuardError::Problem(ShipProblem::Uncommitted { files }) => AppError::conflict(format!(
            "Cannot accept: the workspace has changes beyond the hand-in commit ({}). Return the \
             card so the agent commits or discards them, or deal with them on the ship.",
            guard::list_files(&files)
        )),
        GuardError::Problem(ShipProblem::Empty) => AppError::conflict(format!(
            "Cannot accept: the branch has no commit of its own on top of {}.",
            look.start
        )),
        GuardError::Ship(e) => AppError::conflict(format!(
            "Cannot accept: Toknila could not look at the workspace ({e}). Try again."
        )),
    }
}

/// The ship a spec stands for once the shipyard gave it a path.
pub(crate) fn ship_of(ticket: TicketId, spec: &ShipSpec, path: String) -> Ship {
    Ship {
        ticket_id: ticket,
        key: spec.key.clone(),
        branch: spec.branch.clone(),
        path,
    }
}

/// `PrepareShip`: queue making (or reusing) the ticket's ship for after the commit.
pub(crate) fn queue_prepare<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    ctx: &ChainTx,
    ticket: TicketId,
    stale: StalePolicy,
) -> AppResult<()> {
    if let Some(mut spec) = spec_for(uow, ctx, ticket)? {
        spec.stale = stale;
        uow.defer(ExternalEffect::PrepareShip { ticket, spec });
    }
    Ok(())
}

/// `RemoveShip`: queue taking the worktree away for after the commit; a ticket without a ship is
/// left alone.
pub(crate) fn queue_remove<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    ctx: &ChainTx,
    ticket: TicketId,
) -> AppResult<()> {
    let (Some(ship), Some(spec)) = (uow.ship(ticket)?, spec_for(uow, ctx, ticket)?) else {
        return Ok(());
    };
    uow.defer(ExternalEffect::RemoveShip {
        repo: spec.repo,
        ship,
    });
    Ok(())
}

/// `RetireShip`: queue taking a cancelled ticket's worktree away (and its branch, if empty) for
/// after the commit; a ticket without a ship is left alone.
pub(crate) fn queue_retire<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    ctx: &ChainTx,
    ticket: TicketId,
) -> AppResult<()> {
    let (Some(ship), Some(spec)) = (uow.ship(ticket)?, spec_for(uow, ctx, ticket)?) else {
        return Ok(());
    };
    uow.defer(ExternalEffect::RetireShip {
        start: spec.start_point(),
        repo: spec.repo,
        ship,
    });
    Ok(())
}

/// The cancelled ship is gone: forget its path, journal it, and when its branch had work on it
/// say so on the card.
pub(crate) fn retired<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    ship: Ship,
    fate: BranchFate,
) -> AppResult<()> {
    let (ticket, branch) = (ship.ticket_id, ship.branch.clone());
    removed(uow, ship)?;
    let BranchFate::Kept { ahead } = fate else {
        return Ok(());
    };
    let Some(t) = uow.ticket(ticket)? else {
        return Ok(());
    };
    let Some(view) = uow.chain_view(t.chain_id)? else {
        return Ok(());
    };
    let body = CommentBody {
        gist: format!("Branch {branch} kept: it has {ahead} commit(s) of work"),
        what: Some(format!(
            "The worktree was removed with the cancel; the branch {branch} was kept because it holds {ahead} commit(s) not in the base."
        )),
        how_to_verify: Some(format!(
            "git log {branch}; delete it with Clean up in Settings when the work is not needed."
        )),
        ..CommentBody::default()
    };
    add_comment(uow, &view, ticket, Actor::System, CommentKind::Report, body).map(|_| ())
}

/// The shipyard made the ship: remember it, journal it, tell the ticket its branch.
pub(crate) fn ready<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    ticket: TicketId,
    ship: Ship,
) -> AppResult<()> {
    if uow.ship(ticket)?.as_ref() == Some(&ship) {
        return Ok(());
    }
    uow.save_ship(&ship)?;
    uow.emit(&DomainEvent::ShipCreated { ship: ship.clone() })?;
    tell_ticket(
        uow,
        ticket,
        TicketEvent::ShipReady {
            branch: ship.branch,
        },
    )
}

/// The shipyard could not make the ship: the ticket is blocked with `ship_failed`.
pub(crate) fn failed<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    ticket: TicketId,
    error: &ShipError,
) -> AppResult<()> {
    let reason = match error {
        ShipError::BaseBehind(b) => BlockedReason::BaseBehind {
            base: b.base.clone(),
            upstream: b.upstream.clone(),
            behind: b.behind,
            can_fast_forward: b.can_fast_forward,
        },
        _ => BlockedReason::ShipFailed {
            detail: error.to_string(),
        },
    };
    tell_ticket(uow, ticket, TicketEvent::Block { reason })
}

/// The worktree is gone: forget its path and journal it. The branch stays.
pub(crate) fn removed<U: UnitOfWork + ?Sized>(uow: &mut U, ship: Ship) -> AppResult<()> {
    uow.delete_ship(ship.ticket_id)?;
    uow.emit(&DomainEvent::ShipRemoved { ship })
}

/// The worktree could not be removed: the journal says so; the ticket and its accept stand.
pub(crate) fn remove_failed<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    ship: Ship,
    error: &ShipError,
) -> AppResult<()> {
    uow.emit(&DomainEvent::ShipRemoveFailed {
        ship,
        detail: error.to_string(),
    })
}
