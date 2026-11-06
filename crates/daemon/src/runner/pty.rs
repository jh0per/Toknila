//! The pseudo-terminal itself (`portable-pty`: Unix ptys and Windows ConPTY). Nothing else in the
//! workspace touches it.

use std::io::{Read, Write};

use portable_pty::{Child, ChildKiller, CommandBuilder, MasterPty, PtySize, native_pty_system};
use toknila_app::RunnerError;
use toknila_core::{AgentProfile, ProcessExit};

/// Terminal size an agent sees. Fixed for now; resizing arrives with the web terminal.
const SIZE: PtySize = PtySize {
    rows: 40,
    cols: 160,
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
pub(super) fn spawn(profile: &AgentProfile, dir: &str) -> Result<Spawned, RunnerError> {
    let Some((program, args)) = profile.command.split_first() else {
        return Err(RunnerError::NoCommand(profile.name.clone()));
    };
    let pair = native_pty_system().openpty(SIZE).map_err(spawn_error)?;
    let mut cmd = CommandBuilder::new(program);
    cmd.args(args);
    cmd.cwd(dir);
    for (k, v) in &profile.env {
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
