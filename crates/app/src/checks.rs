//! The check of a hand-in: when a worker says `finished`, the daemon runs the project's check
//! command on the ship itself instead of trusting the agent. Core decides when (the card waits
//! in the `Check` turn, the `RunCheck` effect); a [`Checker`] port runs the command on the host
//! after the command committed ([`crate::external`]); this module queues that work and turns the
//! outcome into an event for the ticket: `CheckDone`, or `CheckLost` when it could not run.

use toknila_core::machine::TicketEvent;
use toknila_core::{ReviewTurn, ShipSpec, Ticket, TicketId, TicketKey, TicketStatus, toolpath};

use crate::chain::tell_ticket;
use crate::external::{ExternalEffect, ExternalOutcome, Host};
use crate::sync::{SyncOutcome, SyncSpec};
use crate::{AppResult, ChainTx, Reader, UnitOfWork, projects, reviews};

/// The most characters of a check's output a round and the worker's instruction keep (the end of
/// it, where the failure is).
pub const OUTPUT_LIMIT: usize = 8000;

/// What the check ran, and where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckSpec {
    /// The project's check command, exactly as the person typed it.
    pub command: String,
    /// The ship's working directory.
    pub dir: String,
}

/// How a check command ended when it ran at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckResult {
    /// The command exited with success.
    pub success: bool,
    /// The exit code; `None` when it was killed (a signal, or the time limit).
    pub code: Option<i32>,
    /// The time limit passed and the command was stopped.
    pub timed_out: bool,
    /// What it wrote, standard output and error together.
    pub output: String,
}

/// Why a check could not run.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CheckError {
    /// The card has no ship, or the project names no command.
    #[error("there is nothing to run the check on")]
    NothingToRun,
    /// The command could not be started.
    #[error("the check command could not start: {0}")]
    Io(String),
    /// The base branch could not be brought into the ship.
    #[error("the ship could not be brought up to its base: {0}")]
    Sync(String),
}

/// What the application needs from the host to check a hand-in. The daemon runs the command in
/// the ship, with a time limit, through the platform's shell.
pub trait Checker: std::fmt::Debug {
    /// Runs `spec.command` in `spec.dir` and waits for it, or stops it at the time limit.
    fn run(&self, spec: &CheckSpec) -> Result<CheckResult, CheckError>;

    /// The host is stopping: end every command that is running (and the ones it started) now.
    fn stop_all(&self) {}
}

/// A check's turn in the line; dropping it hands the turn to the next card.
pub trait CheckTurn {}
impl<T> CheckTurn for T {}

/// The line the checks of a machine stand in: only one runs at a time, across projects.
pub trait CheckGate: std::fmt::Debug + Sync {
    /// The card joins the line (when it is not in it) and waits for its turn; the turn lasts
    /// until the returned guard is dropped.
    fn enter(&self, ticket: TicketId) -> Box<dyn CheckTurn + '_>;
}

/// A gate with no line: every check starts at once (tests of the use cases, no host).
#[derive(Debug, Default, Clone, Copy)]
pub struct OpenGate;

impl CheckGate for OpenGate {
    fn enter(&self, _ticket: TicketId) -> Box<dyn CheckTurn + '_> {
        Box::new(())
    }
}

/// What a check tells its card while it works.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckStage {
    /// The base branch is being brought into the ship.
    Syncing,
    /// The check command `command` started.
    Running { command: String },
}

/// Where a check reports its stage; the daemon records it on the card as an event.
pub trait CheckProgress: std::fmt::Debug + Sync {
    /// The check of `ticket` reached `stage`.
    fn stage(&self, ticket: TicketId, stage: CheckStage);
}

/// A progress port that tells nobody (tests of the use cases, no host).
#[derive(Debug, Default, Clone, Copy)]
pub struct NoProgress;

impl CheckProgress for NoProgress {
    fn stage(&self, _ticket: TicketId, _stage: CheckStage) {}
}

/// `RunCheck` on the host: waits for the machine's one turn, brings the base into the ship, runs
/// the command, and reports each stage on the way. Touches no database.
pub(crate) fn run_on_host(
    host: &Host<'_>,
    ticket: TicketId,
    spec: Option<CheckSpec>,
    sync: Option<&SyncSpec>,
    outcomes: &mut Vec<ExternalOutcome>,
) {
    // One check at a time on the machine: wait for the turn, hold it until done.
    let _turn = host.gate.enter(ticket);
    let synced = match sync {
        Some(sync) => {
            host.progress.stage(ticket, CheckStage::Syncing);
            host.merger.sync(sync)
        }
        None => Ok(SyncOutcome::UpToDate),
    };
    outcomes.extend(crate::sync::recorded(ticket, &synced));
    let result = match (&spec, synced) {
        (_, Ok(SyncOutcome::Conflict { files, migrations })) => {
            let base = sync.map(|s| s.base.as_str()).unwrap_or_default();
            let text = crate::sync::conflict_text(base, &files, &migrations);
            outcomes.push(ExternalOutcome::SyncConflict { ticket, text });
            return;
        }
        (_, Err(e)) => Err(CheckError::Sync(e.to_string())),
        (Some(spec), Ok(_)) => {
            let command = spec.command.clone();
            host.progress.stage(ticket, CheckStage::Running { command });
            host.checker.run(spec)
        }
        (None, Ok(_)) => Err(CheckError::NothingToRun),
    };
    outcomes.push(ExternalOutcome::CheckFinished {
        ticket,
        spec,
        result,
    });
}

/// A stage of a check arrived: the card learns it. A card that has moved on ignores it.
pub fn stage_reached<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    ticket: TicketId,
    stage: CheckStage,
) -> AppResult<()> {
    let event = match stage {
        CheckStage::Syncing => TicketEvent::CheckSyncing,
        CheckStage::Running { command } => TicketEvent::CheckRunning { command },
    };
    tell_ticket(uow, ticket, event)
}

/// Whether a hand-in of `t` is checked by the daemon, and the project's round limit. A card is
/// checked (brought up to its base, and the command run when there is one) whenever it has a ship.
pub fn plan_of<R: Reader + ?Sized>(r: &R, t: &Ticket) -> AppResult<(bool, u32)> {
    let project = projects::of_ticket(r, t)?;
    let check = r.ship(t.id)?.is_some();
    Ok((check, project.max_review_rounds))
}

/// `RunCheck`: queue running the project's check command for after the commit.
pub(crate) fn queue_run<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    ctx: &ChainTx,
    ticket: TicketId,
) -> AppResult<()> {
    let Some(t) = ctx.view.tickets.iter().find(|t| t.id == ticket) else {
        return Ok(());
    };
    queue_for(uow, t)
}

fn queue_for<U: UnitOfWork + ?Sized>(uow: &mut U, t: &Ticket) -> AppResult<()> {
    let ticket = t.id;
    let project = projects::of_ticket(uow, t)?;
    let ship = uow.ship(ticket)?;
    // The base goes into the ship before the check runs, when the base moved.
    let sync = match (&ship, uow.chain(t.chain_id)?) {
        (Some(ship), Some(chain)) => ShipSpec::for_ticket(&project, &chain, t).map(|spec| {
            let merge = ShipSpec {
                branch: ship.branch.clone(),
                ..spec
            }
            .merge_spec(&t.title, &ship.path);
            SyncSpec::of(&merge)
        }),
        _ => None,
    };
    let dir = ship.map(|s| s.path);
    uow.defer(ExternalEffect::RunCheck {
        ticket,
        spec: project
            .check_command
            .zip(dir)
            .map(|(command, dir)| CheckSpec { command, dir }),
        sync,
    });
    Ok(())
}

/// The daemon merged the base into the ship and made `commit`: it is recorded with the hand-in,
/// so Accept knows it for the daemon's own and not for work that was not handed in.
pub(crate) fn synced<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    ticket: TicketId,
    commit: String,
) -> AppResult<()> {
    let Some(t) = uow.ticket(ticket)? else {
        return Ok(());
    };
    crate::chain::with_chain(uow, t.chain_id, |ctx| {
        if let Some(mine) = ctx.view.tickets.iter_mut().find(|x| x.id == ticket)
            && !mine.review.synced.contains(&commit)
        {
            mine.review.synced.push(commit);
        }
        Ok::<_, crate::AppError>(())
    })
}

/// Bringing the ship up to its base conflicted: the card goes back to the worker with `text`, a
/// round of the loop, like a check that failed.
pub(crate) fn sync_conflict<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    ticket: TicketId,
    text: String,
) -> AppResult<()> {
    let Some(t) = uow.ticket(ticket)? else {
        return Ok(());
    };
    let event = TicketEvent::CheckDone {
        passed: false,
        output: text,
        mode: reviews::mode_of(uow, &t)?,
        max_rounds: projects::of_ticket(uow, &t)?.max_review_rounds,
    };
    tell_ticket(uow, ticket, event)
}

/// What a worker reads (and a round keeps) about a check that ran: the command, how it ended and
/// the end of its output.
pub fn report(command: &str, r: &CheckResult) -> String {
    let how = if r.timed_out {
        "did not finish in time and was stopped".to_owned()
    } else if let Some(code) = r.code {
        format!("exited with code {code}")
    } else {
        "was killed".to_owned()
    };
    let output = tail(r.output.trim_end(), OUTPUT_LIMIT);
    format!("`{command}` {how}.\n\n```\n{output}\n```")
}

/// The last `limit` characters of `text`, with a mark when the start was cut.
fn tail(text: &str, limit: usize) -> String {
    let count = text.chars().count();
    if count <= limit {
        return text.to_owned();
    }
    let cut: String = text.chars().skip(count - limit).collect();
    format!("[... {} characters cut ...]\n{cut}", count - limit)
}

/// The check ended on the host: the card learns the result through the machine. A card that has
/// moved on meanwhile ignores it.
pub(crate) fn finished<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    ticket: TicketId,
    command: Option<&str>,
    result: Result<CheckResult, CheckError>,
) -> AppResult<()> {
    let Some(t) = uow.ticket(ticket)? else {
        return Ok(());
    };
    let event = match result {
        // No command: only the sync ran, and it went through.
        Err(CheckError::NothingToRun) if command.is_none() => TicketEvent::CheckSkipped {
            mode: reviews::mode_of(uow, &t)?,
        },
        // The program was not found: the machine's problem, not a round of the loop.
        Ok(r)
            if !r.success
                && !r.timed_out
                && command.is_some()
                && toolpath::command_not_found(
                    r.code,
                    &tail(r.output.trim_end(), OUTPUT_LIMIT),
                ) =>
        {
            TicketEvent::CheckCannotRun {
                command: command.unwrap_or_default().to_owned(),
            }
        }
        Ok(r) => TicketEvent::CheckDone {
            passed: r.success,
            output: report(command.unwrap_or_default(), &r),
            mode: reviews::mode_of(uow, &t)?,
            max_rounds: projects::of_ticket(uow, &t)?.max_review_rounds,
        },
        Err(e) => TicketEvent::CheckLost {
            detail: e.to_string(),
        },
    };
    tell_ticket(uow, ticket, event)
}

/// After a daemon start: a card still in the `Check` turn lost its place in the line (or its
/// running check) with the process, so it is queued again, oldest hand-in first. Running a
/// check twice is safe. Returns their keys, in the order they are queued.
pub fn resume<U: UnitOfWork + ?Sized>(uow: &mut U) -> AppResult<Vec<TicketKey>> {
    let mut waiting: Vec<Ticket> = Vec::new();
    for chain in uow.chains(None)? {
        waiting.extend(uow.tickets(Some(chain.id))?.into_iter().filter(|t| {
            t.status == TicketStatus::Review && t.review.turn == Some(ReviewTurn::Check)
        }));
    }
    waiting.sort_by_key(|t| (t.status_since, t.key.to_string()));
    let mut resumed = Vec::new();
    for t in waiting {
        queue_for(uow, &t)?;
        resumed.push(t.key);
    }
    Ok(resumed)
}
