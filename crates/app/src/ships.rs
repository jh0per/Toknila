//! Ships: the worktree and branch a ticket works in. Core decides when one is needed (the
//! `PrepareShip` and `RemoveShip` effects) and what it is called ([`ShipSpec`]); a [`Shipyard`]
//! port makes and removes them after the command committed ([`crate::external`]); this module
//! queues that work and records what came of it.

use toknila_core::events::DomainEvent;
use toknila_core::guard::{self, ShipProblem, ShipState};
use toknila_core::machine::TicketEvent;
use toknila_core::{
    BaseBehind, BlockedReason, Chain, PlannerSite, PlannerSpec, Ship, ShipLook, ShipSpec,
    StalePolicy, Ticket, TicketId,
};

use crate::external::ExternalEffect;

use crate::chain::tell_ticket;
use crate::{AppError, AppResult, ChainTx, Reader, UnitOfWork};

/// Why a ship could not be made or removed. The text lands in the ticket's blocked reason.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ShipError {
    /// The `git` executable is not installed or not on the path.
    #[error("git is not installed or not on PATH")]
    GitMissing,
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
    /// A git command failed.
    #[error("git {command} failed: {detail}")]
    Git { command: String, detail: String },
    /// The file system refused.
    #[error("{0}")]
    Io(String),
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
    /// Looks at the worktree at `look.path`: the changes in it that are not committed, and the
    /// commits its branch has beyond `look.start`. A yard that cannot look says the ship holds
    /// one clean commit, so nothing is refused for lack of an answer.
    fn inspect(&self, look: &ShipLook) -> Result<ShipState, ShipError> {
        let _ = look;
        Ok(ShipState {
            changed: Vec::new(),
            ahead: 1,
        })
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
    let state = yard.inspect(look).map_err(GuardError::Ship)?;
    guard::judge(&state).map_err(GuardError::Problem)
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
