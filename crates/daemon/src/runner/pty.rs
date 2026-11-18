//! The pseudo-terminal itself (`portable-pty`: Unix ptys and Windows ConPTY). Nothing else in the
//! workspace touches it.

use std::io::{Read, Write};
use std::path::Path;
use std::time::Duration;

use portable_pty::{Child, ChildKiller, CommandBuilder, MasterPty, PtySize, native_pty_system};
use toknila_app::RunnerError;
use toknila_core::instructions::terminal;
use toknila_core::{AgentProfile, ProcessExit};

use super::drain::Slave;
use super::wiring::Wiring;

/// The size a terminal starts with, until a client fits it to its panel.
const SIZE: PtySize = PtySize {
    rows: terminal::DEFAULT.rows,
    cols: terminal::DEFAULT.cols,
    pixel_width: 0,
    pixel_height: 0,
};

/// A started process and the ends of its terminal.
pub(super) struct Spawned {
    pub reader: Box<dyn Read + Send>,
    pub writer: Box<dyn Write + Send>,
    pub killer: Box<dyn ChildKiller + Send + Sync>,
    pub child: Box<dyn Child + Send + Sync>,
    /// Dropping it closes the terminal; the reader then reaches its end.
    pub master: Box<dyn MasterPty + Send>,
    /// A slave end kept open for the run, to tell the reader the output is complete.
    pub slave: Slave,
}

impl std::fmt::Debug for Spawned {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Spawned")
    }
}

/// Starts the profile's command (an argument list, never a shell string) in `dir`.
pub(super) fn spawn(
    profile: &AgentProfile,
    dir: &Path,
    wiring: &Wiring,
) -> Result<Spawned, RunnerError> {
    let Some((program, args)) = wiring.command.split_first() else {
        return Err(RunnerError::NoCommand(profile.name.clone()));
    };
    let pair = open_with(|| native_pty_system().openpty(SIZE), OPEN_RETRY_WAIT)?;
    let mut cmd = CommandBuilder::new(program);
    cmd.args(args);
    cmd.args(&wiring.args);
    cmd.cwd(dir);
    for name in &wiring.env_remove {
        cmd.env_remove(name);
    }
    for (k, v) in &profile.env {
        cmd.env(k, v);
    }
    for (k, v) in &wiring.env {
        cmd.env(k, v);
    }
    // Opened before the child starts, so a slave end is open from the first byte it prints.
    let slave = Slave::open(pair.master.as_ref());
    let child = pair.slave.spawn_command(cmd).map_err(spawn_error)?;
    drop(pair.slave);
    Ok(Spawned {
        slave,
        reader: pair.master.try_clone_reader().map_err(spawn_error)?,
        writer: pair.master.take_writer().map_err(spawn_error)?,
        killer: child.clone_killer(),
        child,
        master: pair.master,
    })
}

/// How long to wait before the one retry of a pty that could not be opened.
const OPEN_RETRY_WAIT: Duration = Duration::from_millis(300);

/// A pty the operating system refused to open is a state of the machine (ENXIO on a loaded Mac,
/// "Unknown error: -6"; no descriptor left elsewhere): `NoPty`, with what the system said.
fn open_error<E: std::fmt::Display>(e: &E) -> RunnerError {
    RunnerError::NoPty(format!("{e:#}"))
}

/// Opens a pty with `open`, once more after `wait` when it fails, then gives up with `NoPty`.
fn open_with<T, E: std::fmt::Display>(
    mut open: impl FnMut() -> Result<T, E>,
    wait: Duration,
) -> Result<T, RunnerError> {
    match open() {
        Ok(opened) => Ok(opened),
        Err(_) => {
            std::thread::sleep(wait);
            open().map_err(|e| open_error(&e))
        }
    }
}

fn spawn_error<E: std::fmt::Display>(e: E) -> RunnerError {
    RunnerError::Spawn(format!("{e:#}"))
}

/// What ConPTY prints when it starts: "where is the cursor?" (`ESC [ 6 n`). It holds the process
/// back until the terminal answers, and the daemon is the only terminal there is until a client
/// attaches, so it answers once, at the top left.
const CURSOR_QUERY: &[u8] = b"\x1b[6n";
/// The answer: row 1, column 1.
pub(super) const CURSOR_REPLY: &[u8] = b"\x1b[1;1R";
/// Bytes of the previous read to keep, so a query split between two reads is still seen.
pub(super) const QUERY_OVERLAP: usize = CURSOR_QUERY.len() - 1;

/// Whether `output` contains the cursor query.
pub(super) fn asks_for_cursor(output: &[u8]) -> bool {
    output
        .windows(CURSOR_QUERY.len())
        .any(|w| w == CURSOR_QUERY)
}

/// How the process ended.
pub(super) fn exit_of(status: &portable_pty::ExitStatus) -> ProcessExit {
    let signal = status.signal().map(str::to_owned);
    ProcessExit {
        code: signal
            .is_none()
            .then(|| i32::try_from(status.exit_code()).unwrap_or(i32::MAX)),
        signal,
    }
}

/// Whether a size asked for waits (see [`Pending`]): on Windows, a smaller screen, or any size
/// while an earlier one still waits (the latest wins, in order); never with no `settle` time.
pub(super) fn waits(current: PtySize, asked: PtySize, pending: bool, settle: Duration) -> bool {
    cfg!(windows)
        && !settle.is_zero()
        && (pending || asked.rows < current.rows || asked.cols < current.cols)
}

/// [`SETTLE`], or the milliseconds in `TOKNILA_RESIZE_SETTLE_MS` when `TOKNILA_TEST_ROOT` is set
/// too (a test daemon; `0` sends every size at once, as before TOKNILA-162). A real daemon
/// ignores the variable.
pub(super) fn settle() -> Duration {
    if std::env::var_os("TOKNILA_TEST_ROOT").is_none() {
        return SETTLE;
    }
    std::env::var("TOKNILA_RESIZE_SETTLE_MS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .map_or(SETTLE, Duration::from_millis)
}

/// Gives `master` the size `size`, or holds it in `pending` when it [`waits`].
pub(super) fn resize(
    master: &(dyn MasterPty + Send),
    pending: &Pending,
    size: terminal::Size,
) -> Result<(), RunnerError> {
    let size = PtySize {
        rows: size.rows,
        cols: size.cols,
        pixel_width: 0,
        pixel_height: 0,
    };
    let now = master.get_size().unwrap_or(size);
    if waits(now, size, pending.waiting(), settle()) {
        pending.ask(size);
        return Ok(());
    }
    master
        .resize(size)
        .map_err(|e| RunnerError::Write(e.to_string()))
}

/// Applies each size `pending` lets through to the terminal in `master` (while it is open),
/// until `over()`. `quiet()` is how long the reader has had nothing.
pub(super) fn apply_sizes(
    pending: &Pending,
    master: &std::sync::Mutex<Option<Box<dyn MasterPty + Send>>>,
    quiet: impl Fn() -> Duration,
    over: impl Fn() -> bool,
) {
    let lock = || {
        master
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    };
    let closed = || over() || lock().is_none();
    while let Some(size) = pending.next(settle(), SETTLE_AT_MOST, &quiet, closed) {
        if let Some(Err(e)) = lock().as_ref().map(|m| m.resize(size)) {
            tracing::warn!(error = %e, "cannot resize the terminal");
        }
    }
}

/// How long the output must have been quiet before a smaller screen reaches ConPTY, and the
/// longest a size waits for that (a program that prints without a pause still gets it).
pub(super) const SETTLE: Duration = Duration::from_millis(300);
pub(super) const SETTLE_AT_MOST: Duration = Duration::from_secs(2);

/// A size a client asked for that has not reached the terminal yet. ConPTY drops the rows above
/// a shrunk screen that it has not rendered: the run log of the terminal-copy spec lacked lines
/// 1-12 and 1-8 in two of six Windows runs (CI run 44317344931), when the page sent its size while
/// the agent was still printing. Once the reader has had nothing for [`SETTLE`], ConPTY has
/// rendered what the program printed, and the shrink drops only rows already in the log.
#[derive(Debug, Default)]
pub(super) struct Pending {
    /// The latest size, and when the oldest size not yet applied was asked for.
    size: std::sync::Mutex<Option<(PtySize, std::time::Instant)>>,
    wake: std::sync::Condvar,
}

impl Pending {
    /// Holds `size` for [`Pending::next`]; it replaces one still waiting, which keeps its time.
    pub fn ask(&self, size: PtySize) {
        let mut held = self
            .size
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let since = held.map_or_else(std::time::Instant::now, |(_, t)| t);
        *held = Some((size, since));
        self.wake.notify_all();
    }

    /// Whether a size is waiting.
    pub fn waiting(&self) -> bool {
        let held = self
            .size
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        held.is_some()
    }

    /// The next size to apply, once `quiet()` (how long the reader has had nothing) reaches
    /// `settle` or the size has waited `at_most`; `None` once `over()`. `quiet` and `over` are
    /// called without the lock held: a resize holds the terminal's lock and then takes this one,
    /// so taking them the other way round here would deadlock (CI run 44327828545).
    pub fn next(
        &self,
        settle: Duration,
        at_most: Duration,
        quiet: impl Fn() -> Duration,
        over: impl Fn() -> bool,
    ) -> Option<PtySize> {
        loop {
            if over() {
                return None;
            }
            let quiet_enough = quiet() >= settle;
            let mut held = self
                .size
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some((size, since)) = *held
                && (quiet_enough || since.elapsed() >= at_most)
            {
                *held = None;
                return Some(size);
            }
            drop(
                self.wake
                    .wait_timeout(held, Duration::from_millis(20))
                    .unwrap_or_else(std::sync::PoisonError::into_inner),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_open_that_the_os_refuses_is_no_pty_whatever_it_says() {
        // What portable-pty reports on a Mac with no free pty (ENXIO), and a Linux style one.
        for text in [
            "failed to openpty: Unknown error: -6",
            "failed to openpty: Os { code: 6, kind: Uncategorized, message: \"Device not configured\" }",
            "failed to openpty: Os { code: 24, kind: Uncategorized, message: \"Too many open files\" }",
        ] {
            let err = open_error(&std::io::Error::other(text.to_owned()));
            assert!(
                matches!(&err, RunnerError::NoPty(d) if d == text),
                "{err:?}"
            );
        }
    }

    #[test]
    fn a_failed_open_is_tried_once_more_and_then_given_up() {
        let mut calls = 0;
        let got: Result<u8, RunnerError> = open_with(
            || {
                calls += 1;
                Err(std::io::Error::other(
                    "failed to openpty: Unknown error: -6",
                ))
            },
            std::time::Duration::from_millis(1),
        );
        assert!(matches!(got, Err(RunnerError::NoPty(_))), "{got:?}");
        assert_eq!(calls, 2, "one try and one retry");
        let mut calls = 0;
        let got = open_with(
            || {
                calls += 1;
                if calls == 1 {
                    Err(std::io::Error::other(
                        "failed to openpty: Unknown error: -6",
                    ))
                } else {
                    Ok(7u8)
                }
            },
            std::time::Duration::from_millis(1),
        );
        assert_eq!((got.ok(), calls), (Some(7), 2), "the retry succeeds");
        let mut calls = 0;
        let got = open_with(
            || {
                calls += 1;
                Ok::<u8, std::io::Error>(1)
            },
            std::time::Duration::from_millis(1),
        );
        assert_eq!((got.ok(), calls), (Some(1), 1), "no retry when it opens");
    }

    #[test]
    fn the_cursor_query_is_found_anywhere_in_the_output() {
        assert!(asks_for_cursor(b"\x1b[6n"));
        assert!(asks_for_cursor(b"\x1b[?9001h\x1b[6nhello"));
        assert!(!asks_for_cursor(b"\x1b[6"));
        assert!(!asks_for_cursor(b"plain output"));
    }

    fn size(cols: u16, rows: u16) -> PtySize {
        PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        }
    }

    #[test]
    fn only_a_smaller_screen_on_windows_waits_and_then_every_size_after_it() {
        let (big, small) = (size(160, 40), size(152, 14));
        let settle = SETTLE;
        assert_eq!(waits(big, small, false, settle), cfg!(windows));
        assert_eq!(waits(big, size(150, 40), false, settle), cfg!(windows));
        assert!(
            !waits(small, big, false, settle),
            "a larger screen drops nothing"
        );
        assert_eq!(waits(small, big, true, settle), cfg!(windows), "in order");
        assert!(
            !waits(big, small, true, Duration::ZERO),
            "no settle time: at once"
        );
    }

    /// `next` on its own thread, for a test that must not block without a deadline.
    fn next_later(
        pending: std::sync::Arc<Pending>,
        settle: Duration,
        at_most: Duration,
        quiet_ms: std::sync::Arc<std::sync::atomic::AtomicU64>,
    ) -> std::sync::mpsc::Receiver<Option<PtySize>> {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let quiet =
                || Duration::from_millis(quiet_ms.load(std::sync::atomic::Ordering::SeqCst));
            let _ = tx.send(pending.next(settle, at_most, quiet, || false));
        });
        rx
    }

    #[test]
    fn a_waiting_size_goes_through_once_the_output_is_quiet_the_latest_one() {
        use std::sync::{Arc, atomic::AtomicU64, atomic::Ordering};
        let pending = Arc::new(Pending::default());
        let quiet = Arc::new(AtomicU64::new(0));
        let rx = next_later(
            pending.clone(),
            Duration::from_millis(50),
            Duration::from_secs(60),
            quiet.clone(),
        );
        pending.ask(size(152, 20));
        pending.ask(size(152, 14));
        assert!(pending.waiting());
        assert!(
            rx.recv_timeout(Duration::from_millis(200)).is_err(),
            "the output is not quiet yet"
        );
        quiet.store(60, Ordering::SeqCst);
        let got = rx
            .recv_timeout(Duration::from_secs(10))
            .expect("once quiet");
        assert_eq!(got, Some(size(152, 14)));
        assert!(!pending.waiting());
    }

    #[test]
    fn a_waiting_size_goes_through_after_the_longest_wait_however_busy_the_output() {
        let pending = std::sync::Arc::new(Pending::default());
        let quiet = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
        let at_most = Duration::from_millis(100);
        let rx = next_later(pending.clone(), Duration::from_secs(60), at_most, quiet);
        let asked = std::time::Instant::now();
        pending.ask(size(152, 14));
        let got = rx
            .recv_timeout(Duration::from_secs(10))
            .expect("after the longest wait");
        assert_eq!(got, Some(size(152, 14)));
        assert!(asked.elapsed() >= at_most, "{:?}", asked.elapsed());
    }

    #[test]
    fn a_run_that_is_over_applies_nothing() {
        let pending = Pending::default();
        pending.ask(size(152, 14));
        assert_eq!(
            pending.next(Duration::ZERO, Duration::ZERO, || Duration::MAX, || true),
            None
        );
    }

    #[test]
    fn a_query_split_between_reads_is_found_with_the_kept_overlap() {
        let (first, second) = (&b"abc\x1b[6"[..], &b"n"[..]);
        let mut window = first[first.len() - QUERY_OVERLAP..].to_vec();
        window.extend_from_slice(second);
        assert!(asks_for_cursor(&window));
    }
}
