//! The check of a hand-in: when a worker says `finished`, the daemon runs the project's check
//! command on the ship itself instead of trusting the agent. Core decides when (the card waits
//! in the `Check` turn, the `RunCheck` effect); a [`Checker`] port runs the command on the host
//! after the command committed ([`crate::external`]); this module queues that work and turns the
//! outcome into an event for the ticket: `CheckDone`, or `CheckLost` when it could not run.

use std::collections::HashSet;

use toknila_core::machine::TicketEvent;
use toknila_core::{ReviewTurn, ShipSpec, Ticket, TicketId, TicketKey, TicketStatus};

use crate::chain::tell_ticket;
use crate::external::ExternalEffect;
use crate::sync::SyncSpec;
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
        Err(CheckError::NothingToRun) if command.is_none() => TicketEvent::CheckDone {
            passed: true,
            output: "The ship was brought up to its base; the project has no check command."
                .to_owned(),
            mode: reviews::mode_of(uow, &t)?,
            max_rounds: projects::of_ticket(uow, &t)?.max_review_rounds,
        },
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

/// After a daemon start: a card still being checked lost its check with the process, and goes to
/// a person (`live` is empty today: checks do not outlive the daemon). Returns their keys.
pub fn mark_lost<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    live: &HashSet<TicketId>,
) -> AppResult<Vec<TicketKey>> {
    let mut moved = Vec::new();
    for chain in uow.chains(None)? {
        let lost: Vec<(TicketId, TicketKey)> = uow
            .tickets(Some(chain.id))?
            .iter()
            .filter(|t| t.status == TicketStatus::Review)
            .filter(|t| t.review.turn == Some(ReviewTurn::Check))
            .filter(|t| !live.contains(&t.id))
            .map(|t| (t.id, t.key.clone()))
            .collect();
        for (id, key) in lost {
            let detail = "the daemon restarted while it was checking".to_owned();
            tell_ticket(uow, id, TicketEvent::CheckLost { detail })?;
            moved.push(key);
        }
    }
    Ok(moved)
}
