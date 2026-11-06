//! One run: a process in a terminal, its output on its way to the log, the scrollback and the
//! stream, and the watcher that reports how it ended.

use std::fs::File;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use portable_pty::ChildKiller;
use tokio::sync::{broadcast, mpsc};
use toknila_app::runner::{RunEnded, RunnerError};
use toknila_core::{ProcessExit, Run, TicketId};

use super::Chunk;
use super::pty::{self, Spawned};
use super::scrollback::Scrollback;

/// How long the watcher waits for the last output after the process ended.
const DRAIN: Duration = Duration::from_secs(2);

#[derive(Debug)]
struct State {
    scrollback: Scrollback,
    /// Set once, when the process has ended and its output is complete.
    exit: Option<ProcessExit>,
}

/// A run, live or finished.
pub(super) struct Active {
    run: Run,
    state: Mutex<State>,
    writer: Mutex<Box<dyn Write + Send>>,
    killer: Mutex<Box<dyn ChildKiller + Send + Sync>>,
    /// A stop was requested, so the exit is not a crash.
    stopped: AtomicBool,
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
        log: File,
        lines: usize,
        wake: broadcast::Sender<TicketId>,
        ended: mpsc::UnboundedSender<RunEnded>,
    ) -> Result<Arc<Active>, RunnerError> {
        let Spawned {
            reader,
            writer,
            killer,
            mut child,
            master,
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
                    // The reader ends when the terminal closes. On Unix output the process left
                    // behind stays readable until then, so read it first. ConPTY flushes late and
                    // reports the end only once its console is closed, so close it after a pause.
                    if cfg!(windows) {
                        std::thread::sleep(Duration::from_millis(200));
                        drop(master);
                        let _ = drained.recv_timeout(DRAIN);
                    } else {
                        let _ = drained.recv_timeout(DRAIN);
                        drop(master);
                    }
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
        mut log: File,
        wake: &broadcast::Sender<TicketId>,
    ) {
        let mut buf = vec![0; 8192];
        let mut log_failed = false;
        loop {
            let n = match reader.read(&mut buf) {
                Ok(0) | Err(_) => return,
                Ok(n) => n,
            };
            // The log first: whatever a subscriber is told is already on disk.
            if let Err(e) = log.write_all(&buf[..n]).and_then(|()| log.flush())
                && !log_failed
            {
                log_failed = true;
                tracing::warn!(run = %self.run.id, error = %e, "cannot write the run log");
            }
            lock(&self.state).scrollback.push(&buf[..n]);
            let _ = wake.send(self.run.ticket_id);
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

    /// Kills the process on purpose; its exit then is not a crash.
    pub fn kill(&self) {
        if self.is_over() {
            return;
        }
        self.stopped.store(true, Ordering::SeqCst);
        let _ = lock(&self.killer).kill();
    }
}
