//! The pseudo-terminal itself (`portable-pty`: Unix ptys and Windows ConPTY). Nothing else in the
//! workspace touches it.

use std::io::{Read, Write};
use std::path::Path;

use portable_pty::{Child, ChildKiller, CommandBuilder, MasterPty, PtySize, native_pty_system};
use toknila_app::RunnerError;
use toknila_core::instructions::terminal;
use toknila_core::{AgentProfile, ProcessExit};

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
    let Some((program, args)) = profile.command.split_first() else {
        return Err(RunnerError::NoCommand(profile.name.clone()));
    };
    let pair = native_pty_system().openpty(SIZE).map_err(spawn_error)?;
    let mut cmd = CommandBuilder::new(program);
    cmd.args(args);
    cmd.args(&wiring.args);
    cmd.cwd(dir);
    for (k, v) in &profile.env {
        cmd.env(k, v);
    }
    for (k, v) in &wiring.env {
        cmd.env(k, v);
    }
    let child = pair.slave.spawn_command(cmd).map_err(spawn_error)?;
    // The child holds the only slave end now; with ours closed, the reader ends when it exits.
    drop(pair.slave);
    Ok(Spawned {
        reader: pair.master.try_clone_reader().map_err(spawn_error)?,
        writer: pair.master.take_writer().map_err(spawn_error)?,
        killer: child.clone_killer(),
        child,
        master: pair.master,
    })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cursor_query_is_found_anywhere_in_the_output() {
        assert!(asks_for_cursor(b"\x1b[6n"));
        assert!(asks_for_cursor(b"\x1b[?9001h\x1b[6nhello"));
        assert!(!asks_for_cursor(b"\x1b[6"));
        assert!(!asks_for_cursor(b"plain output"));
    }

    #[test]
    fn a_query_split_between_reads_is_found_with_the_kept_overlap() {
        let (first, second) = (&b"abc\x1b[6"[..], &b"n"[..]);
        let mut window = first[first.len() - QUERY_OVERLAP..].to_vec();
        window.extend_from_slice(second);
        assert!(asks_for_cursor(&window));
    }
}
