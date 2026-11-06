//! The PTY runner: the daemon side of `toknila_app::Runner`. It runs a profile's command in a
//! pseudo-terminal in the ticket's ship, keeps the output (a bounded in-memory scrollback and a
//! full log file under `<home>/runs/<ticket>/<run>.log`), wakes stream subscribers, and reports
//! the process's end so the application can move the ticket (`supervise`).
//!
//! No database here and nothing runs inside a transaction: the store calls [`Runners`] after a
//! command committed. `portable-pty` lives in `pty` and nowhere else.

mod active;
mod logs;
mod pty;
mod scrollback;
mod supervise;
mod tokens;
mod wiring;
mod ws;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use tokio::sync::{broadcast, mpsc};
use toknila_app::runner::{RunEnded, RunRequest, Runner, RunnerError, RunnerState, Started};
use toknila_core::instructions::terminal::Size;
use toknila_core::{AgentProfile, OutputRules, ProcessExit, Run, RunId, TicketId};

use active::Active;
pub use supervise::supervise;
pub use tokens::{Activity, AgentTokens, Grant};
pub use ws::stream;

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
    active: Mutex<HashMap<TicketId, Arc<Active>>>,
    /// Pings with the ticket whose output or state changed.
    wake: broadcast::Sender<TicketId>,
    ended: mpsc::UnboundedSender<RunEnded>,
    /// One token per live run, and the agents' last signs of life.
    tokens: Arc<AgentTokens>,
    /// The daemon's address, known once it listens; agents get it in their environment.
    url: OnceLock<String>,
    /// The `toknila` binary agents run for `mcp` and `hook`; this process unless set.
    bin: OnceLock<PathBuf>,
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
            url: OnceLock::new(),
            bin: OnceLock::new(),
        };
        (Arc::new(runners), rx)
    }

    /// The agent tokens of the live runs.
    pub fn tokens(&self) -> &Arc<AgentTokens> {
        &self.tokens
    }

    /// Tells runners where the daemon listens, so agents can reach it. Set once.
    pub fn set_daemon_url(&self, url: String) {
        let _ = self.url.set(url);
    }

    /// Sets the `toknila` binary agents run for `mcp` and `hook`. Set once.
    pub fn set_agent_bin(&self, bin: PathBuf) {
        let _ = self.bin.set(bin);
    }

    fn agent_bin(&self) -> PathBuf {
        self.bin
            .get()
            .cloned()
            .or_else(|| std::env::current_exe().ok())
            .unwrap_or_else(|| PathBuf::from("toknila"))
    }

    /// Pings whenever a ticket's runner wrote or ended.
    pub fn subscribe(&self) -> broadcast::Receiver<TicketId> {
        self.wake.subscribe()
    }

    fn active(&self) -> std::sync::MutexGuard<'_, HashMap<TicketId, Arc<Active>>> {
        self.active.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Tickets with a live process.
    pub fn live(&self) -> Vec<TicketId> {
        self.active()
            .iter()
            .filter(|(_, a)| !a.is_over())
            .map(|(t, _)| *t)
            .collect()
    }

    /// Kills every live process, for a daemon that is shutting down.
    pub fn stop_all(&self) {
        for ticket in self.live() {
            self.stop(ticket);
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

    /// The scrollback of the ticket's newest run from `cursor` on (the same run, at an offset)
    /// or from its first retained byte. A ticket this daemon has not run is read from its newest
    /// log. `None` when the ticket never ran.
    pub fn read(&self, ticket: TicketId, cursor: Option<(RunId, u64)>) -> Option<Chunk> {
        let active = self.active().get(&ticket).cloned();
        if let Some(a) = active {
            return Some(a.read(cursor));
        }
        let (run, path) = logs::latest(&self.root, ticket)?;
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
        let cx = wiring::Context {
            dir: &dir,
            key: &req.key,
            run: run.id,
            token,
            url: self.url.get().map(String::as_str),
            bin: &bin,
            brief: &req.brief,
        };
        let wiring = wiring::prepare(profile, &cx)?;
        let process = pty::spawn(profile, Path::new(&req.dir), &wiring)?;
        let log =
            logs::RunLog::create(&self.root, req.ticket, run.id, profile.output.max_log_bytes)
                .map_err(|e| RunnerError::Spawn(format!("cannot create the run log: {e}")))?;
        let lines = usize::try_from(profile.output.scrollback_lines).unwrap_or(usize::MAX);
        Active::launch(
            run.clone(),
            process,
            log,
            lines,
            profile.hooks.enabled && profile.hooks.settings_flag.is_some(),
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
        let mut map = self.active();
        if let Some(a) = map.get(&req.ticket).filter(|a| !a.is_over()) {
            return Ok(Started {
                run: a.run().clone(),
                reused: true,
            });
        }
        let run = Run {
            id: RunId::new(),
            ticket_id: req.ticket,
            key: req.key.clone(),
            profile: profile.name.clone(),
            dir: req.dir.clone(),
        };
        let grant = Grant {
            ticket: req.ticket,
            run: run.id,
        };
        let token = self
            .tokens
            .issue(grant, crate::server::now())
            .map_err(RunnerError::Spawn)?;
        let active = self
            .launch(&profile, req, &run, &token)
            .inspect_err(|_| self.tokens.revoke(run.id))?;
        map.insert(req.ticket, active);
        Ok(Started { run, reused: false })
    }

    fn stop(&self, ticket: TicketId) {
        let active = self.active().get(&ticket).cloned();
        if let Some(a) = active {
            a.kill();
        }
    }

    fn write(&self, ticket: TicketId, bytes: &[u8]) -> Result<(), RunnerError> {
        let active = self.active().get(&ticket).cloned();
        match active.filter(|a| !a.is_over()) {
            Some(a) => a.write(bytes),
            None => Err(RunnerError::NotRunning),
        }
    }

    fn state(&self, ticket: TicketId) -> Option<RunnerState> {
        let active = self.active().get(&ticket).cloned();
        active?.state()
    }

    fn resize(&self, ticket: TicketId, size: Size) -> Result<(), RunnerError> {
        let active = self.active().get(&ticket).cloned();
        match active.filter(|a| !a.is_over()) {
            Some(a) => a.resize(size),
            None => Err(RunnerError::NotRunning),
        }
    }
}
