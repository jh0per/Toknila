//! One run: a process in a terminal, its output on its way to the log, the scrollback and the
//! stream, and the watcher that reports how it ended.

use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use portable_pty::{ChildKiller, MasterPty, PtySize};
use tokio::sync::{broadcast, mpsc};
use toknila_app::runner::{RunEnded, RunnerError, RunnerState};
use toknila_core::instructions::terminal::Size;
use toknila_core::{ProcessExit, Run, TicketId};

use super::Chunk;
use super::drain::MarkFilter;
use super::logs::RunLog;
use super::pty::{self, Spawned};
use super::scrollback::Scrollback;
use super::tokens::AgentTokens;
use super::tree;

/// How long a terminal prints nothing before its agent counts as waiting at its prompt. A working
/// agent keeps its spinner and output moving.
const IDLE_AFTER: Duration = Duration::from_secs(3);

/// How long the watcher waits for the last output after the process ended.
const DRAIN: Duration = Duration::from_secs(2);

#[derive(Debug)]
struct State {
    scrollback: Scrollback,
    /// Set once, when the process has ended and its output is complete.
    exit: Option<ProcessExit>,
}

/// What a run reports to: the stream's wake-ups, the end of the process, the agent tokens.
pub(super) struct Channels {
    pub wake: broadcast::Sender<TicketId>,
    pub ended: mpsc::UnboundedSender<RunEnded>,
    pub tokens: Arc<AgentTokens>,
}

/// A run, live or finished.
pub(super) struct Active {
    run: Run,
    state: Mutex<State>,
    writer: Mutex<Box<dyn Write + Send>>,
    killer: Mutex<Box<dyn ChildKiller + Send + Sync>>,
    /// A stop was requested, so the exit is not a crash.
    stopped: AtomicBool,
    /// The terminal itself, for resizing; taken (and so closed) once the process has ended.
    master: Mutex<Option<Box<dyn MasterPty + Send>>>,
    /// When the process last printed.
    last_output: Mutex<Instant>,
    /// The agent's Stop hook reports to Toknila.
    hooks: bool,
    /// The agent's process id, while it is known.
    pid: Option<u32>,
    /// The `toknila` binary the agent runs for its MCP server and hooks; not work of its own.
    helper: PathBuf,
}

impl std::fmt::Debug for Active {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Active").field("run", &self.run.id).finish()
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Active {
    /// Starts the threads that move output and watch for the exit.
    pub fn launch(
        run: Run,
        process: Spawned,
        log: RunLog,
        lines: usize,
        hooks: bool,
        helper: PathBuf,
        channels: Channels,
    ) -> Result<Arc<Active>, RunnerError> {
        let Channels {
            wake,
            ended,
            tokens,
        } = channels;
        let Spawned {
            reader,
            writer,
            killer,
            mut child,
            master,
            #[cfg_attr(windows, allow(unused_mut, unused_variables))]
            mut slave,
        } = process;
        let active = Arc::new(Active {
            run,
            state: Mutex::new(State {
                scrollback: Scrollback::new(lines),
                exit: None,
            }),
            writer: Mutex::new(writer),
            killer: Mutex::new(killer),
            stopped: AtomicBool::new(false),
            master: Mutex::new(Some(master)),
            last_output: Mutex::new(Instant::now()),
            hooks,
            pid: child.process_id(),
            helper,
        });
        let (done, drained) = channel();
        let pump = {
            let (a, wake) = (active.clone(), wake.clone());
            std::thread::Builder::new()
                .name("runner-read".to_owned())
                .spawn(move || {
                    a.pump(reader, log, &wake);
                    let _ = done.send(());
                })
        };
        let watch = {
            let a = active.clone();
            std::thread::Builder::new()
                .name("runner-wait".to_owned())
                .spawn(move || {
                    let exit = child.wait().map_or(
                        ProcessExit {
                            code: None,
                            signal: None,
                        },
                        |s| pty::exit_of(&s),
                    );
                    // On Unix the reader ends at the mark written through our slave end, after
                    // everything the process printed (see `drain`); close the terminal after that.
                    // ConPTY flushes late and reports the end only once its console is closed, so
                    // close it after a pause.
                    if cfg!(windows) {
                        std::thread::sleep(Duration::from_millis(200));
                        drop(lock(&a.master).take());
                        let _ = drained.recv_timeout(DRAIN);
                    } else {
                        slave.mark();
                        let _ = drained.recv_timeout(DRAIN);
                        drop(slave);
                        drop(lock(&a.master).take());
                    }
                    // The run is over: its token stops working before anyone hears about the end.
                    tokens.revoke(a.run.id);
                    a.finish(exit, &wake, &ended);
                })
        };
        if let Err(e) = pump.and(watch) {
            active.kill();
            return Err(RunnerError::Spawn(format!("cannot start threads: {e}")));
        }
        Ok(active)
    }

    /// Moves what the process prints to the log, the scrollback and the subscribers.
    fn pump(
        &self,
        mut reader: Box<dyn Read + Send>,
        mut log: RunLog,
        wake: &broadcast::Sender<TicketId>,
    ) {
        let mut buf = vec![0; 8192];
        let mut log_failed = false;
        let mut answered_cursor = !cfg!(windows);
        let mut tail: Vec<u8> = Vec::new();
        let mut marks = MarkFilter::default();
        let mut keep = |bytes: &[u8]| {
            if bytes.is_empty() {
                return;
            }
            // The log first: whatever a subscriber is told is already on disk.
            if let Err(e) = log.write(bytes)
                && !log_failed
            {
                log_failed = true;
                tracing::warn!(run = %self.run.id, error = %e, "cannot write the run log");
            }
            *lock(&self.last_output) = Instant::now();
            lock(&self.state).scrollback.push(bytes);
            let _ = wake.send(self.run.ticket_id);
        };
        loop {
            let n = match reader.read(&mut buf) {
                Ok(0) | Err(_) => return keep(&marks.rest()),
                Ok(n) => n,
            };
            if !answered_cursor {
                tail.extend_from_slice(&buf[..n]);
                if pty::asks_for_cursor(&tail) {
                    answered_cursor = true;
                    if let Err(e) = self.write(pty::CURSOR_REPLY) {
                        tracing::warn!(run = %self.run.id, error = %e, "cannot answer the cursor query");
                    }
                } else {
                    tail.drain(..tail.len().saturating_sub(pty::QUERY_OVERLAP));
                }
            }
            let (output, complete) = marks.feed(&buf[..n]);
            keep(&output);
            if complete {
                return;
            }
        }
    }

    fn finish(
        &self,
        exit: ProcessExit,
        wake: &broadcast::Sender<TicketId>,
        ended: &mpsc::UnboundedSender<RunEnded>,
    ) {
        lock(&self.state).exit = Some(exit.clone());
        let _ = wake.send(self.run.ticket_id);
        let _ = ended.send(RunEnded {
            run: self.run.clone(),
            exit,
            stopped: self.stopped.load(Ordering::SeqCst),
        });
    }

    pub fn run(&self) -> &Run {
        &self.run
    }

    /// The process has ended and its output is complete.
    pub fn is_over(&self) -> bool {
        lock(&self.state).exit.is_some()
    }

    /// The output from `cursor` (same run) or from the first retained byte.
    pub fn read(&self, cursor: Option<(toknila_core::RunId, u64)>) -> Chunk {
        let state = lock(&self.state);
        let from = cursor
            .filter(|(run, _)| *run == self.run.id)
            .map_or(0, |(_, at)| at);
        let (offset, bytes) = state.scrollback.from(from);
        Chunk {
            run: self.run.id,
            offset,
            bytes: bytes.to_vec(),
            end: state.scrollback.end(),
            ended: state.exit.is_some(),
            exit: state.exit.clone(),
        }
    }

    pub fn write(&self, bytes: &[u8]) -> Result<(), RunnerError> {
        let mut writer = lock(&self.writer);
        writer
            .write_all(bytes)
            .and_then(|()| writer.flush())
            .map_err(|e| RunnerError::Write(e.to_string()))
    }

    /// How to reach the agent, or `None` once it is over.
    pub fn state(&self) -> Option<RunnerState> {
        (!self.is_over()).then(|| {
            let quiet = lock(&self.last_output).elapsed();
            RunnerState {
                hooks: self.hooks,
                idle: quiet >= IDLE_AFTER,
                quiet_ms: u64::try_from(quiet.as_millis()).unwrap_or(u64::MAX),
                busy: self.pid.is_some_and(|pid| tree::busy(pid, &self.helper)),
            }
        })
    }

    /// Gives the terminal a new size; the agent gets the window-size signal.
    pub fn resize(&self, size: Size) -> Result<(), RunnerError> {
        let master = lock(&self.master);
        let master = master.as_ref().ok_or(RunnerError::NotRunning)?;
        master
            .resize(PtySize {
                rows: size.rows,
                cols: size.cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| RunnerError::Write(e.to_string()))
    }

    /// Ends the process and everything it started, on purpose; its exit then is not a crash.
    pub fn kill(&self) {
        if self.is_over() {
            return;
        }
        self.stopped.store(true, Ordering::SeqCst);
        if !self.pid.is_some_and(tree::terminate) {
            let _ = lock(&self.killer).kill();
        }
    }
}
