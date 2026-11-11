//! The PTY runner: the daemon side of `toknila_app::Runner`. It runs a profile's command in a
//! pseudo-terminal in the ticket's ship, keeps the output (a bounded in-memory scrollback and a
//! full log file under `<home>/runs/<ticket>/<run>.log`), wakes stream subscribers, and reports
//! the process's end so the application can move the ticket (`supervise`).
//!
//! A ticket has one runner per role: its worker, and while the card is in review a reviewer next
//! to it, on the same ship. A project's planner is one more runner, keyed by the project's id
//! read as a card id (`toknila_core::planner::run_key`), in the project's repository. The worker's output is what the ticket's routes mean; the reviewer's
//! is read, written to and answered through the `reviewer` role selector.
//!
//! No database here and nothing runs inside a transaction: the store calls [`Runners`] after a
//! command committed. `portable-pty` lives in `pty` and nowhere else.

mod active;
mod drain;
mod facts;
mod logs;
mod private;
mod prompts;
mod pty;
mod reviewer;
mod screens;
mod scrollback;
mod silence;
mod stopping;
mod supervise;
mod tokens;
mod tree;
mod wiring;
mod ws;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::Duration;

use tokio::sync::{broadcast, mpsc};
use toknila_app::runner::{RunEnded, RunRequest, Runner, RunnerError, RunnerState, Started};
use toknila_core::instructions::terminal::Size;
use toknila_core::{AgentProfile, AgentRole, OutputRules, ProcessExit, Run, RunId, TicketId};

use active::Active;
pub use prompts::Prompts;
pub use silence::{tick as silence_tick, watch as watch_silence};
pub use supervise::supervise;
pub use tokens::{Activity, AgentTokens, Grant, SpawnFacts};
pub use ws::{planner_stream, stream};

/// What a client sees of a ticket's output: the scrollback of its newest run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chunk {
    pub run: RunId,
    /// Offset of the first byte in the run's output.
    pub offset: u64,
    pub bytes: Vec<u8>,
    /// Offset after the last byte so far.
    pub end: u64,
    /// The process has ended and `end` is final. `exit` says how, when this daemon saw it.
    pub ended: bool,
    pub exit: Option<ProcessExit>,
}

/// Every runner this daemon started, by ticket: the live one, or the last one that ended.
#[derive(Debug)]
pub struct Runners {
    /// `<home>/runs`.
    root: PathBuf,
    /// The profiles folder, read when a runner starts so an edit applies at once.
    profiles: PathBuf,
    active: Mutex<HashMap<(TicketId, AgentRole), Arc<Active>>>,
    /// Pings with the ticket whose output or state changed.
    wake: broadcast::Sender<TicketId>,
    ended: mpsc::UnboundedSender<RunEnded>,
    /// One token per live run, and the agents' last signs of life.
    tokens: Arc<AgentTokens>,
    /// The last tool of each run and the questions that wait for a person.
    prompts: Prompts,
    /// How long a permission hook is held for an answer, in milliseconds.
    permission_wait_ms: AtomicU64,
    /// The daemon's address, known once it listens; agents get it in their environment.
    url: OnceLock<String>,
}

impl Runners {
    /// Runners that keep logs under `root` and read profiles from `profiles`. The receiver gets
    /// every process end; [`supervise`] feeds it to the store.
    pub fn new(
        root: PathBuf,
        profiles: PathBuf,
    ) -> (Arc<Runners>, mpsc::UnboundedReceiver<RunEnded>) {
        let (ended, rx) = mpsc::unbounded_channel();
        let (wake, _) = broadcast::channel(256);
        let runners = Runners {
            root,
            profiles,
            active: Mutex::default(),
            wake,
            ended,
            tokens: Arc::default(),
            prompts: Prompts::default(),
            permission_wait_ms: AtomicU64::new(toknila_core::hook::PERMISSION_WAIT_SECS * 1000),
            url: OnceLock::new(),
        };
        (Arc::new(runners), rx)
    }

    /// The agent tokens of the live runs.
    pub fn tokens(&self) -> &Arc<AgentTokens> {
        &self.tokens
    }

    /// The questions of the live runs that the daemon holds in memory.
    pub fn prompts(&self) -> &Prompts {
        &self.prompts
    }

    /// How long a permission hook is held for the person's answer.
    pub fn permission_wait(&self) -> Duration {
        Duration::from_millis(self.permission_wait_ms.load(Ordering::Relaxed))
    }

    /// Sets how long a permission hook is held (tests shorten it).
    pub fn set_permission_wait(&self, wait: Duration) {
        let ms = u64::try_from(wait.as_millis()).unwrap_or(u64::MAX);
        self.permission_wait_ms.store(ms, Ordering::Relaxed);
    }

    /// Tells runners where the daemon listens, so agents can reach it. Set once.
    pub fn set_daemon_url(&self, url: String) {
        let _ = self.url.set(url);
    }

    /// The `toknila` binary agents run for `mcp` and `hook`: this process.
    fn agent_bin(&self) -> PathBuf {
        std::env::current_exe().unwrap_or_else(|_| PathBuf::from("toknila"))
    }

    /// Pings whenever a ticket's runner wrote or ended.
    pub fn subscribe(&self) -> broadcast::Receiver<TicketId> {
        self.wake.subscribe()
    }

    fn active(&self) -> std::sync::MutexGuard<'_, HashMap<(TicketId, AgentRole), Arc<Active>>> {
        self.active.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Tickets with a live worker process.
    pub fn live(&self) -> Vec<TicketId> {
        self.live_in(AgentRole::Worker)
    }

    /// Tickets with a live process of `role`.
    pub fn live_in(&self, role: AgentRole) -> Vec<TicketId> {
        self.active()
            .iter()
            .filter(|((_, r), a)| *r == role && !a.is_over() && !a.stopping())
            .map(|((t, _), _)| *t)
            .collect()
    }

    /// The run of `role` that is alive for `ticket` now, if any.
    pub fn live_run(&self, ticket: TicketId, role: AgentRole) -> Option<Run> {
        let active = self.active().get(&(ticket, role)).cloned();
        active
            .filter(|a| !a.is_over() && !a.stopping())
            .map(|a| a.run().clone())
    }

    /// Kills every live process, for a daemon that is shutting down.
    pub fn stop_all(&self) {
        let all: Vec<Arc<Active>> = self.active().values().cloned().collect();
        for a in all {
            a.kill();
        }
    }

    /// Forgets a deleted card: its ended runs and its folder of run logs and generated files
    /// (`runs/<ticket>`). A card whose process is still alive is kept as it is.
    pub fn forget(&self, ticket: TicketId) -> std::io::Result<()> {
        let mut active = self.active();
        if active
            .iter()
            .any(|((t, _), a)| *t == ticket && !a.is_over())
        {
            return Err(std::io::Error::other("its agent is still running"));
        }
        active.retain(|(t, _), _| *t != ticket);
        drop(active);
        match std::fs::remove_dir_all(self.root.join(ticket.to_string())) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }

    fn profile(&self, name: &str) -> Result<AgentProfile, RunnerError> {
        crate::profiles::load(&self.profiles)
            .profiles
            .into_iter()
            .map(|p| p.profile)
            .find(|p| p.name == name)
            .ok_or_else(|| RunnerError::UnknownProfile(name.to_owned()))
    }

    /// The nudge settings of the profile the ticket's live worker run was started with.
    fn nudge_of(&self, ticket: TicketId) -> Option<toknila_core::Nudge> {
        let profile = self
            .active()
            .get(&(ticket, AgentRole::Worker))?
            .run()
            .profile
            .clone();
        self.profile(&profile).ok().map(|p| p.nudge)
    }

    /// The scrollback of the ticket's newest run of `role` from `cursor` on (the same run, at an
    /// offset) or from its first retained byte. A ticket this daemon has not run is read from its
    /// newest log. `None` when the ticket never ran in that role.
    pub fn read(
        &self,
        ticket: TicketId,
        role: AgentRole,
        cursor: Option<(RunId, u64)>,
    ) -> Option<Chunk> {
        let active = self.active().get(&(ticket, role)).cloned();
        if let Some(a) = active {
            return Some(a.read(cursor));
        }
        let (run, path) = logs::latest(&self.root, ticket, role)?;
        let kept = logs::load(
            &path,
            usize::try_from(OutputRules::default().scrollback_lines).unwrap_or(usize::MAX),
        )
        .ok()?;
        let from = cursor.filter(|(r, _)| *r == run).map_or(0, |(_, at)| at);
        let (offset, bytes) = kept.from(from);
        Some(Chunk {
            run,
            offset,
            bytes: bytes.to_vec(),
            end: kept.end(),
            ended: true,
            exit: None,
        })
    }
}

impl Runners {
    /// Wires the agent, starts its process and the threads that watch it.
    fn launch(
        &self,
        profile: &AgentProfile,
        req: &RunRequest,
        run: &Run,
        token: &str,
    ) -> Result<Arc<Active>, RunnerError> {
        let dir = self.root.join(req.ticket.to_string());
        let bin = self.agent_bin();
        let mut brief = req.brief.clone();
        if req.role == AgentRole::Reviewer {
            brief.repo_tools = wiring::repo_tools(Path::new(&req.dir));
        }
        let cx = wiring::Context {
            dir: &dir,
            key: &req.key,
            role: req.role,
            run: run.id,
            token,
            url: self.url.get().map(String::as_str),
            bin: &bin,
            brief: &brief,
        };
        let wiring = wiring::prepare(profile, &cx)?;
        let process = pty::spawn(profile, Path::new(&req.dir), &wiring)?;
        let log = logs::RunLog::create(
            &self.root,
            req.ticket,
            req.role,
            run.id,
            profile.output.max_log_bytes,
        )
        .map_err(|e| RunnerError::Spawn(format!("cannot create the run log: {e}")))?;
        let lines = usize::try_from(profile.output.scrollback_lines).unwrap_or(usize::MAX);
        Active::launch(
            run.clone(),
            process,
            log,
            lines,
            profile.hooks.enabled && profile.hooks.settings_flag.is_some(),
            bin,
            active::Channels {
                wake: self.wake.clone(),
                ended: self.ended.clone(),
                tokens: self.tokens.clone(),
            },
        )
    }
}

impl Runner for Runners {
    fn start(&self, req: &RunRequest) -> Result<Started, RunnerError> {
        let profile = self.profile(&req.profile)?;
        stopping::wait_out(self, req);
        let mut map = self.active();
        if let Some(a) = map
            .get(&(req.ticket, req.role))
            .filter(|a| !a.is_over() && !a.stopping())
        {
            return Ok(Started {
                run: a.run().clone(),
                reused: true,
                briefed: false,
            });
        }
        let run = Run {
            id: RunId::new(),
            ticket_id: req.ticket,
            key: req.key.clone(),
            profile: profile.name.clone(),
            dir: req.dir.clone(),
            role: req.role,
        };
        let grant = Grant {
            ticket: req.ticket,
            run: run.id,
            role: req.role,
        };
        let token = self
            .tokens
            .issue(grant, crate::server::now())
            .map_err(RunnerError::Spawn)?;
        let active = self
            .launch(&profile, req, &run, &token)
            .inspect_err(|_| self.tokens.revoke(run.id))?;
        // A run that reports to Toknila (hooks or MCP) and shows nothing for `startup_secs` waits
        // on its screen; one that never reports cannot be told apart from a working one.
        let reports = profile.hooks.enabled || profile.mcp.enabled;
        let startup = if reports { profile.startup_secs } else { 0 };
        self.prompts.reset(req.ticket, req.role);
        self.tokens.spawned(grant, crate::server::now(), startup);
        map.insert((req.ticket, req.role), active);
        Ok(Started {
            run,
            reused: false,
            briefed: profile.reads_brief(),
        })
    }

    fn stop(&self, ticket: TicketId) {
        for role in [AgentRole::Worker, AgentRole::Reviewer] {
            self.stop_role(ticket, role);
        }
    }

    fn live_runners(&self) -> Vec<(TicketId, AgentRole)> {
        self.active()
            .iter()
            .filter(|(_, a)| !a.is_over())
            .map(|(key, _)| *key)
            .collect()
    }

    fn stop_role(&self, ticket: TicketId, role: AgentRole) {
        let active = self.active().get(&(ticket, role)).cloned();
        if let Some(a) = active {
            a.kill();
        }
    }

    fn write(&self, ticket: TicketId, role: AgentRole, bytes: &[u8]) -> Result<(), RunnerError> {
        let active = self.active().get(&(ticket, role)).cloned();
        match active.filter(|a| !a.is_over() && !a.stopping()) {
            Some(a) => a.write(bytes),
            None => Err(RunnerError::NotRunning),
        }
    }

    fn state(&self, ticket: TicketId) -> Option<RunnerState> {
        let active = self.active().get(&(ticket, AgentRole::Worker)).cloned();
        active.filter(|a| !a.stopping())?.state()
    }

    fn resize(&self, ticket: TicketId, size: Size) -> Result<(), RunnerError> {
        self.resize_role(ticket, AgentRole::Worker, size)
    }

    fn resize_role(
        &self,
        ticket: TicketId,
        role: AgentRole,
        size: Size,
    ) -> Result<(), RunnerError> {
        let active = self.active().get(&(ticket, role)).cloned();
        match active.filter(|a| !a.is_over() && !a.stopping()) {
            Some(a) => a.resize(size),
            None => Err(RunnerError::NotRunning),
        }
    }
}
