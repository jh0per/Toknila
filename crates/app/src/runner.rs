//! The runner port: what the application needs from the host to run an agent's command in a
//! terminal. The daemon implements it with pseudo-terminals. Core decides when a runner starts
//! and stops (the `EnsureRunner` and `StopRunner` effects); [`crate::runs`] queues that work and
//! records what came of it, and [`crate::external`] calls this port after the command committed.

use toknila_core::instructions::terminal::Size;
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

/// What to run: the ticket's agent in its working directory. A planner run is keyed by its
/// project's id (`toknila_core::planner::run_key`) and works in the project's repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunRequest {
    pub ticket: TicketId,
    pub key: TicketKey,
    /// Worker, reviewer or planner. A ticket has at most one live runner per role: the reviewer runs
    /// next to the worker and never replaces it.
    pub role: AgentRole,
    /// Name of the agent profile whose command runs.
    pub profile: String,
    /// The ship's path.
    pub dir: String,
    pub brief: RunBrief,
}

/// What the agent is told about its ticket.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RunBrief {
    pub title: String,
    pub prompt: String,
    pub acceptance: Vec<String>,
    /// The project's check command, named in the agent's instructions.
    pub check_command: Option<String>,
    /// The project's setup command: the host runs it once in a new ship before the agent starts,
    /// and the agent is told it ran.
    pub setup_command: Option<String>,
    /// Instructions the card's earlier runs were given (`toknila_core::instructions::earlier`),
    /// for a run that replaces one.
    pub earlier: Vec<String>,
    /// The card's discussion with the assistant (`toknila_core::discussion::transcript`).
    pub discussion: Vec<String>,
    /// Instructions no run has acted on yet (`toknila_core::instructions::waiting`): the run
    /// reads them first and they count as delivered once it started.
    pub waiting: Vec<String>,
    /// Every pending instruction comment the run takes over, including one that only repeats the
    /// Return; the host marks them delivered when it started a new process.
    pub waiting_ids: Vec<toknila_core::CommentId>,
    /// The Return this run answers.
    pub returned: Option<toknila_core::instructions::Returned>,
    /// The card must show a design for approval before any code.
    pub design_first: bool,
    /// The designs the card references (the host turns them into files the agent may read).
    pub card_designs: Vec<toknila_core::design::DesignRef>,
    /// The project whose library holds `card_designs`.
    pub design_project: Option<toknila_core::ProjectId>,
    /// The project's Claude model (`--model`), for a profile that runs Claude Code.
    pub model: Option<String>,
    /// The project's setting for when a card's agent wants to run a command (a worker only).
    pub command_policy: toknila_core::CommandPolicy,
    /// The rules a person remembered in the project with "Allow always" (a worker only).
    pub allowed_commands: Vec<String>,
    /// The domains the project's reviewer may read on the web (a reviewer only).
    pub web_domains: Vec<String>,
    /// The toolchains the ship's repository holds (`cargo`, `npm`), which a reviewer may run the
    /// tests of; filled in by the host when the reviewer starts.
    pub repo_tools: Vec<String>,
    /// What a planner is told about its project; `None` for a card's agent.
    pub project: Option<toknila_core::instructions::ProjectFacts>,
    /// The absolute paths of the design files a planner may read (a planner only).
    pub designs: Vec<std::path::PathBuf>,
    /// The card's chain as its epic: the chain's name and goal (a card's agent only).
    pub epic: Option<(String, String)>,
    /// What the check command said about the hand-in under review (a reviewer only).
    pub check: Option<toknila_core::CheckOutcome>,
    /// The run continues a card's paused agent (a worker only): see [`Resume`].
    pub resume: Option<Resume>,
}

/// A worker that starts again after its card waited for a person with no process.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Resume {
    /// The agent's session when it paused; `None` starts fresh.
    pub session: Option<String>,
    /// What would have been typed to the live agent: the answer, as the first input of the run.
    /// A run that cannot be resumed (no session, no resume form of the profile, a refused id)
    /// starts fresh and reads it in its kickoff as an instruction that waits.
    pub input: String,
}

/// A runner now works for the ticket.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Started {
    pub run: Run,
    /// A live runner already existed and was kept; nothing new started.
    pub reused: bool,
    /// A new process started and its first message or instructions file carries the run's brief,
    /// including the instructions that waited for it.
    pub briefed: bool,
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
    /// Every runner process that is alive, by ticket and role.
    fn live_runners(&self) -> Vec<(TicketId, AgentRole)>;
    /// [`Runner::live_runners`] with the time each process started; the epoch when unknown.
    fn live_runs(&self) -> Vec<toknila_core::schedule::LiveRun> {
        self.live_runners()
            .into_iter()
            .map(|(t, r)| (t, r, toknila_core::Timestamp(0)))
            .collect()
    }
    /// Writes bytes to the terminal of the ticket's runner of `role`.
    fn write(&self, ticket: TicketId, role: AgentRole, bytes: &[u8]) -> Result<(), RunnerError>;
    /// What a live run is like, for choosing how to reach it; `None` without a live process.
    fn state(&self, ticket: TicketId) -> Option<RunnerState>;
    /// Sets the size of the ticket's terminal. The caller has clamped it.
    fn resize(&self, ticket: TicketId, size: Size) -> Result<(), RunnerError>;
    /// Sets the size of the terminal of `ticket`'s runner of `role`. The caller has clamped it.
    fn resize_role(&self, ticket: TicketId, role: AgentRole, size: Size)
    -> Result<(), RunnerError>;
}

/// A live run, as far as reaching it goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunnerState {
    /// Its Stop hook reports to Toknila.
    pub hooks: bool,
    /// It has printed nothing for a while: it waits at its prompt.
    pub idle: bool,
    /// Milliseconds since it last printed.
    pub quiet_ms: u64,
    /// It has a live child process of its own (a build, tests), apart from Toknila's helpers.
    pub busy: bool,
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

    fn live_runners(&self) -> Vec<(TicketId, AgentRole)> {
        Vec::new()
    }

    fn write(&self, _: TicketId, _: AgentRole, _: &[u8]) -> Result<(), RunnerError> {
        Err(RunnerError::Unavailable)
    }

    fn state(&self, _: TicketId) -> Option<RunnerState> {
        None
    }

    fn resize(&self, _: TicketId, _: Size) -> Result<(), RunnerError> {
        Err(RunnerError::Unavailable)
    }

    fn resize_role(&self, _: TicketId, _: AgentRole, _: Size) -> Result<(), RunnerError> {
        Err(RunnerError::Unavailable)
    }
}
