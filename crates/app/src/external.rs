//! Work that leaves the database: git and the agents' processes. A command never does it inside
//! its unit of work. It queues [`ExternalEffect`]s ([`crate::UnitOfWork::defer`]); the adapter
//! commits the command, then [`execute`]s them on the host, then runs [`apply`] in a second unit
//! of work so each outcome reaches the ticket as an event through the state machine.

use std::collections::{HashMap, HashSet};

use toknila_core::{AgentRole, Run, Ship, ShipSpec, TicketId, TicketKey};

use crate::runner::{RunRequest, Runner, RunnerError};
use crate::{AppResult, ShipError, Shipyard, UnitOfWork, reviews, runs, ships};

/// Something to do on the host once the command that asked for it has committed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExternalEffect {
    /// Make (or reuse) the ticket's ship.
    PrepareShip { ticket: TicketId, spec: ShipSpec },
    /// Remove the ship's worktree from the repository at `repo`; the branch stays.
    RemoveShip { repo: String, ship: Ship },
    /// Run the profile's command for the ticket in its ship, once the ship is ready.
    StartRunner {
        ticket: TicketId,
        key: TicketKey,
        profile: String,
    },
    /// Stop every runner of the ticket (worker and reviewer).
    StopRunner { ticket: TicketId },
    /// Run the reviewer profile's command in the ticket's ship, next to the worker's runner.
    /// `dir` is the ship's path; `None` when the ticket has no ship.
    StartReviewer {
        ticket: TicketId,
        key: TicketKey,
        profile: String,
        dir: Option<String>,
    },
    /// Stop the reviewer's runner only.
    StopReviewer { ticket: TicketId },
}

/// What happened on the host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExternalOutcome {
    ShipReady {
        ticket: TicketId,
        ship: Ship,
    },
    ShipFailed {
        ticket: TicketId,
        error: ShipError,
    },
    ShipRemoved {
        ship: Ship,
    },
    ShipRemoveFailed {
        ship: Ship,
        error: ShipError,
    },
    /// A new runner process started.
    RunnerStarted {
        run: Run,
    },
    RunnerFailed {
        ticket: TicketId,
        error: RunnerError,
    },
    /// The reviewer's runner could not start.
    ReviewerFailed {
        ticket: TicketId,
        error: RunnerError,
    },
}

/// The host's ports for [`execute`].
#[derive(Debug, Clone, Copy)]
pub struct Host<'a> {
    pub yard: &'a dyn Shipyard,
    pub runner: &'a dyn Runner,
}

/// Runs the effects on the host, in order. Touches no database. A runner starts where the
/// ticket's ship was just made; a ticket whose ship failed gets no runner (it is blocked by
/// that), and one without a ship (its project has no repository) is refused.
pub fn execute(host: Host<'_>, effects: Vec<ExternalEffect>) -> Vec<ExternalOutcome> {
    let mut dirs: HashMap<TicketId, String> = HashMap::new();
    let mut failed: HashSet<TicketId> = HashSet::new();
    let mut outcomes = Vec::new();
    for effect in effects {
        match effect {
            ExternalEffect::PrepareShip { ticket, spec } => match host.yard.prepare(&spec) {
                Ok(path) => {
                    dirs.insert(ticket, path.clone());
                    let ship = ships::ship_of(ticket, &spec, path);
                    outcomes.push(ExternalOutcome::ShipReady { ticket, ship });
                }
                Err(error) => {
                    failed.insert(ticket);
                    outcomes.push(ExternalOutcome::ShipFailed { ticket, error });
                }
            },
            ExternalEffect::RemoveShip { repo, ship } => {
                outcomes.push(match host.yard.remove(&repo, &ship) {
                    Ok(()) => ExternalOutcome::ShipRemoved { ship },
                    Err(error) => ExternalOutcome::ShipRemoveFailed { ship, error },
                });
            }
            ExternalEffect::StartRunner {
                ticket,
                key,
                profile,
            } => {
                if failed.contains(&ticket) {
                    continue;
                }
                let result = dirs
                    .get(&ticket)
                    .ok_or(RunnerError::NoWorkingDir)
                    .and_then(|dir| {
                        let dir = dir.clone();
                        host.runner.start(&RunRequest {
                            ticket,
                            key,
                            role: AgentRole::Worker,
                            profile,
                            dir,
                        })
                    });
                match result {
                    Ok(started) if !started.reused => {
                        outcomes.push(ExternalOutcome::RunnerStarted { run: started.run });
                    }
                    Ok(_) => {}
                    Err(error) => outcomes.push(ExternalOutcome::RunnerFailed { ticket, error }),
                }
            }
            ExternalEffect::StopRunner { ticket } => host.runner.stop(ticket),
            ExternalEffect::StartReviewer {
                ticket,
                key,
                profile,
                dir,
            } => {
                let result = dir.ok_or(RunnerError::NoWorkingDir).and_then(|dir| {
                    host.runner.start(&RunRequest {
                        ticket,
                        key,
                        role: AgentRole::Reviewer,
                        profile,
                        dir,
                    })
                });
                match result {
                    Ok(started) if !started.reused => {
                        outcomes.push(ExternalOutcome::RunnerStarted { run: started.run });
                    }
                    Ok(_) => {}
                    Err(error) => outcomes.push(ExternalOutcome::ReviewerFailed { ticket, error }),
                }
            }
            ExternalEffect::StopReviewer { ticket } => {
                host.runner.stop_role(ticket, AgentRole::Reviewer);
            }
        }
    }
    outcomes
}

/// Records the outcomes: state and journal, with ticket changes going through the machine.
pub fn apply<U: UnitOfWork + ?Sized>(uow: &mut U, outcomes: Vec<ExternalOutcome>) -> AppResult<()> {
    for outcome in outcomes {
        match outcome {
            ExternalOutcome::ShipReady { ticket, ship } => ships::ready(uow, ticket, ship)?,
            ExternalOutcome::ShipFailed { ticket, error } => ships::failed(uow, ticket, &error)?,
            ExternalOutcome::ShipRemoved { ship } => ships::removed(uow, ship)?,
            ExternalOutcome::ShipRemoveFailed { ship, error } => {
                ships::remove_failed(uow, ship, &error)?;
            }
            ExternalOutcome::RunnerStarted { run } => runs::started(uow, run)?,
            ExternalOutcome::RunnerFailed { ticket, error } => {
                runs::start_failed(uow, ticket, &error)?;
            }
            ExternalOutcome::ReviewerFailed { ticket, error } => {
                reviews::start_failed(uow, ticket, &error)?;
            }
        }
    }
    Ok(())
}
