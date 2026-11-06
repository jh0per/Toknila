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
mod ws;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

use tokio::sync::{broadcast, mpsc};
use toknila_app::runner::{RunEnded, RunRequest, Runner, RunnerError, Started};
use toknila_core::{AgentProfile, OutputRules, ProcessExit, Run, RunId, TicketId};

use active::Active;
pub use supervise::supervise;
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
        };
        (Arc::new(runners), rx)
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
        let process = pty::spawn(&profile, &req.dir)?;
        let log = logs::create(&self.root, req.ticket, run.id)
            .map_err(|e| RunnerError::Spawn(format!("cannot create the run log: {e}")))?;
        let lines = usize::try_from(profile.output.scrollback_lines).unwrap_or(usize::MAX);
        let active = Active::launch(
            run.clone(),
            process,
            log,
            lines,
            self.wake.clone(),
            self.ended.clone(),
        )?;
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
}
