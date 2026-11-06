//! The runner port: what the application needs from the host to run an agent's command in a
//! terminal. The daemon implements it with pseudo-terminals. Core decides when a runner starts
//! and stops (the `EnsureRunner` and `StopRunner` effects); [`crate::runs`] queues that work and
//! records what came of it, and [`crate::external`] calls this port after the command committed.

use toknila_core::{AgentRole, ProcessExit, Run, TicketId, TicketKey};

/// Why a runner could not start, or could not be written to. The text lands in the ticket's
/// blocked reason.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RunnerError {
    /// The profile the ticket runs with does not exist (or its file is invalid).
    #[error("agent profile {0} does not exist")]
    UnknownProfile(String),
    /// The profile starts no process (it is the simulated one).
    #[error("agent profile {0} has no command to run")]
    NoCommand(String),
    /// The ticket has no ship and so no directory to run in: its project has no repository.
    #[error("no working directory: the project has no repository, so the ticket has no ship")]
    NoWorkingDir,
    /// The process could not be started.
    #[error("cannot start the agent: {0}")]
    Spawn(String),
    /// The ticket has no live runner.
    #[error("the ticket has no running agent")]
    NotRunning,
    /// The terminal refused the bytes.
    #[error("cannot write to the agent: {0}")]
    Write(String),
    /// This daemon cannot run agents.
    #[error("this daemon cannot run agents")]
    Unavailable,
}

/// What to run: the ticket's agent in its working directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunRequest {
    pub ticket: TicketId,
    pub key: TicketKey,
    /// Worker or reviewer. A ticket has at most one live runner per role: the reviewer runs
    /// next to the worker and never replaces it.
    pub role: AgentRole,
    /// Name of the agent profile whose command runs.
    pub profile: String,
    /// The ship's path.
    pub dir: String,
}

/// A runner now works for the ticket.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Started {
    pub run: Run,
    /// A live runner already existed and was kept; nothing new started.
    pub reused: bool,
}

/// A runner process that has ended, as the host reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunEnded {
    pub run: Run,
    pub exit: ProcessExit,
    /// The host killed it because a stop was requested, so it is not a crash.
    pub stopped: bool,
}

/// Runs agents on the host.
pub trait Runner: std::fmt::Debug {
    /// Starts the profile's command in `req.dir`, or keeps the ticket's live runner of the same
    /// role.
    fn start(&self, req: &RunRequest) -> Result<Started, RunnerError>;
    /// Stops every runner of the ticket (worker and reviewer); none is fine.
    fn stop(&self, ticket: TicketId);
    /// Stops the ticket's runner of one role; the other keeps running.
    fn stop_role(&self, ticket: TicketId, role: AgentRole);
    /// Writes bytes to the ticket's worker terminal. The reviewer's terminal is read-only.
    fn write(&self, ticket: TicketId, bytes: &[u8]) -> Result<(), RunnerError>;
}

/// The runner of a host that has none: every start is refused.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoRunner;

impl Runner for NoRunner {
    fn start(&self, _: &RunRequest) -> Result<Started, RunnerError> {
        Err(RunnerError::Unavailable)
    }

    fn stop(&self, _: TicketId) {}

    fn stop_role(&self, _: TicketId, _: AgentRole) {}

    fn write(&self, _: TicketId, _: &[u8]) -> Result<(), RunnerError> {
        Err(RunnerError::Unavailable)
    }
}
