//! Work that leaves the database: git and the agents' processes. A command never does it inside
//! its unit of work. It queues [`ExternalEffect`]s ([`crate::UnitOfWork::defer`]); the adapter
//! commits the command, then [`execute`]s them on the host, then runs [`apply`] in a second unit
//! of work so each outcome reaches the ticket as an event through the state machine.

use std::collections::{HashMap, HashSet};

use toknila_core::instructions::{self, Route};
use toknila_core::{
    AgentRole, Comment, CommentId, MergeSpec, Run, Ship, ShipSpec, TicketId, TicketKey,
};

use crate::merges::{self, MergeError, MergeOutcome, Merger};
use crate::runner::{RunBrief, RunRequest, Runner, RunnerError};
use crate::{
    AppResult, ShipError, Shipyard, UnitOfWork, instructions as deliver_instructions, reviews,
    runs, ships,
};

/// Something to do on the host once the command that asked for it has committed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExternalEffect {
    /// Make (or reuse) the ticket's ship.
    PrepareShip { ticket: TicketId, spec: ShipSpec },
    /// Merge the ticket's ship branch into the base branch. `None` when the ticket has no ship:
    /// there is nothing to merge, and the host says so.
    Merge {
        ticket: TicketId,
        spec: Option<MergeSpec>,
    },
    /// Remove the ship's worktree from the repository at `repo`; the branch stays.
    RemoveShip { repo: String, ship: Ship },
    /// Run the profile's command for the ticket in its ship, once the ship is ready.
    StartRunner {
        ticket: TicketId,
        key: TicketKey,
        profile: String,
        /// What the agent is told: the ticket's title, task and acceptance.
        brief: RunBrief,
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
        /// What the reviewer is told about the ticket under review.
        brief: RunBrief,
    },
    /// Stop the reviewer's runner only.
    StopReviewer { ticket: TicketId },
    /// Type a message to the live reviewer: it ended a turn without recording its verdict.
    NudgeReviewer { ticket: TicketId, message: String },
    /// Get a person's instruction to the ticket's live agent (see [`crate::instructions`]).
    DeliverInstruction { ticket: TicketId, comment: Comment },
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
    /// The merge ended: it landed, was not needed, or says why it could not.
    MergeFinished {
        ticket: TicketId,
        result: Result<MergeOutcome, MergeError>,
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
    /// The instruction was typed into the agent's terminal.
    InstructionTyped {
        ticket: TicketId,
        comment: CommentId,
    },
}

/// The host's ports for [`execute`].
#[derive(Debug, Clone, Copy)]
pub struct Host<'a> {
    pub yard: &'a dyn Shipyard,
    pub merger: &'a dyn Merger,
    pub runner: &'a dyn Runner,
}

/// Runs the effects on the host, in order. Touches no database. A runner starts where the
/// ticket's ship was just made; a ticket whose ship failed gets no runner (it is blocked by
/// that), and one without a ship (its project has no repository) is refused.
pub fn execute(host: Host<'_>, effects: Vec<ExternalEffect>) -> Vec<ExternalOutcome> {
    let mut dirs: HashMap<TicketId, String> = HashMap::new();
    let mut failed: HashSet<TicketId> = HashSet::new();
    let mut outcomes = Vec::new();
    // Typing an instruction takes a moment, so it goes last: it must not hold up ships and
    // runners that the same command queued, and it sees the runner as they left it.
    let (deliveries, effects): (Vec<_>, Vec<_>) = effects
        .into_iter()
        .partition(|e| matches!(e, ExternalEffect::DeliverInstruction { .. }));
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
            ExternalEffect::Merge { ticket, spec } => {
                let result = match spec {
                    Some(spec) => merges::run(&host, &spec),
                    None => Ok(MergeOutcome::UpToDate),
                };
                outcomes.push(ExternalOutcome::MergeFinished { ticket, result });
            }
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
                brief,
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
                            brief,
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
                brief,
            } => {
                let result = dir.ok_or(RunnerError::NoWorkingDir).and_then(|dir| {
                    host.runner.start(&RunRequest {
                        ticket,
                        key,
                        role: AgentRole::Reviewer,
                        profile,
                        dir,
                        brief,
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
            ExternalEffect::NudgeReviewer { ticket, message } => {
                // The reviewer's turn has just ended; give its terminal a moment to settle.
                std::thread::sleep(ENTER_DELAY);
                type_line(host.runner, ticket, AgentRole::Reviewer, &message);
            }
            ExternalEffect::DeliverInstruction { .. } => {}
        }
    }
    for effect in deliveries {
        if let ExternalEffect::DeliverInstruction { ticket, comment } = effect
            && deliver(host.runner, ticket, &comment)
        {
            outcomes.push(ExternalOutcome::InstructionTyped {
                ticket,
                comment: comment.id,
            });
        }
    }
    outcomes
}

/// Pause between the typed text and Enter.
const ENTER_DELAY: std::time::Duration = std::time::Duration::from_millis(200);

/// Types the instruction into the agent's terminal when that is the way in. Returns whether it
/// was typed. An agent that works with hooks takes it at its next Stop instead, and a ticket
/// without a live runner keeps it pending; neither is an error.
fn deliver(runner: &dyn Runner, ticket: TicketId, comment: &Comment) -> bool {
    let Some(state) = runner.state(ticket) else {
        return false;
    };
    if instructions::route(state.hooks, state.idle) != Route::Type {
        return false;
    }
    type_line(
        runner,
        ticket,
        AgentRole::Worker,
        &instructions::typed(comment),
    )
}

/// The text, then Enter as a separate keystroke: a terminal application takes a newline that
/// arrives with the text as part of a paste. Returns whether both were written.
fn type_line(runner: &dyn Runner, ticket: TicketId, role: AgentRole, line: &str) -> bool {
    runner.write(ticket, role, line.as_bytes()).is_ok() && {
        std::thread::sleep(ENTER_DELAY);
        runner.write(ticket, role, b"\r").is_ok()
    }
}

/// Records the outcomes: state and journal, with ticket changes going through the machine.
pub fn apply<U: UnitOfWork + ?Sized>(uow: &mut U, outcomes: Vec<ExternalOutcome>) -> AppResult<()> {
    for outcome in outcomes {
        match outcome {
            ExternalOutcome::ShipReady { ticket, ship } => ships::ready(uow, ticket, ship)?,
            ExternalOutcome::ShipFailed { ticket, error } => ships::failed(uow, ticket, &error)?,
            ExternalOutcome::MergeFinished { ticket, result } => {
                merges::finished(uow, ticket, result)?;
            }
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
            ExternalOutcome::InstructionTyped { ticket, comment } => {
                deliver_instructions::typed(uow, ticket, comment)?;
            }
        }
    }
    Ok(())
}
