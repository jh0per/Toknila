//! Running `git` for the shipyard and the merger. No shell: every command is the program with an
//! argument list through [`std::process::Command`], so paths with spaces or odd characters pass
//! through untouched on every platform. A missing `git` is [`GitError::Missing`], not a panic.

use std::ffi::OsStr;
use std::io::ErrorKind;
use std::io::Read;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

use toknila_app::{MergeError, ShipError};

#[cfg(test)]
pub(crate) mod testing;

/// Why a git command did not give a result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum GitError {
    /// The program is not installed or not on the path.
    Missing,
    /// The process could not be run.
    Io(String),
    /// The command ran and exited with a failure.
    Failed { command: String, detail: String },
}

impl From<GitError> for ShipError {
    fn from(e: GitError) -> Self {
        match e {
            GitError::Missing => ShipError::GitMissing,
            GitError::Io(text) => ShipError::Io(text),
            GitError::Failed { command, detail } => ShipError::Git { command, detail },
        }
    }
}

impl From<GitError> for MergeError {
    fn from(e: GitError) -> Self {
        match e {
            GitError::Missing => MergeError::GitMissing,
            GitError::Io(text) => MergeError::Io(text),
            GitError::Failed { command, detail } => MergeError::Git { command, detail },
        }
    }
}

/// How long a best-effort `git fetch` may take before it is given up: a network that hangs must
/// not block a start.
pub(crate) const FETCH_TIMEOUT: Duration = Duration::from_secs(20);

/// A `git` executable.
#[derive(Debug, Clone)]
pub(crate) struct Git {
    program: String,
}

impl Git {
    pub(crate) fn new(program: &str) -> Self {
        Self {
            program: program.to_owned(),
        }
    }

    /// Runs git in `repo` and returns whatever it printed, whether it succeeded or not.
    pub(crate) fn output<S: AsRef<OsStr>>(
        &self,
        repo: &str,
        args: &[S],
    ) -> Result<Output, GitError> {
        Command::new(&self.program)
            .arg("-C")
            .arg(repo)
            .args(args)
            .env("GIT_TERMINAL_PROMPT", "0")
            .output()
            .map_err(|e| match e.kind() {
                ErrorKind::NotFound => GitError::Missing,
                _ => GitError::Io(format!("cannot run {}: {e}", self.program)),
            })
    }

    /// Runs git in `repo` like [`Self::run`] and gives up after `limit`: the process is killed and
    /// the answer is a failure saying so.
    pub(crate) fn run_within<S: AsRef<OsStr>>(
        &self,
        repo: &str,
        args: &[S],
        limit: Duration,
    ) -> Result<String, GitError> {
        let command = args
            .iter()
            .map(|a| a.as_ref().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join(" ");
        let mut child = Command::new(&self.program)
            .arg("-C")
            .arg(repo)
            .args(args)
            .env("GIT_TERMINAL_PROMPT", "0")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| match e.kind() {
                ErrorKind::NotFound => GitError::Missing,
                _ => GitError::Io(format!("cannot run {}: {e}", self.program)),
            })?;
        // Read both pipes on threads so a chatty command cannot fill them and stall. Not joined
        // after a kill: a helper process of git may still hold the pipe open.
        fn drain(pipe: Option<impl Read + Send + 'static>) -> std::sync::mpsc::Receiver<Vec<u8>> {
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let mut buf = Vec::new();
                if let Some(mut p) = pipe {
                    let _ = p.read_to_end(&mut buf);
                }
                let _ = tx.send(buf);
            });
            rx
        }
        let out = drain(child.stdout.take());
        let err = drain(child.stderr.take());
        let started = Instant::now();
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) if started.elapsed() >= limit => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(GitError::Failed {
                        command,
                        detail: format!("timed out after {} s", limit.as_secs_f32()),
                    });
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(20)),
                Err(e) => return Err(GitError::Io(format!("cannot wait for git: {e}"))),
            }
        };
        let grace = Duration::from_secs(2);
        let stdout = out.recv_timeout(grace).unwrap_or_default();
        let stderr = err.recv_timeout(grace).unwrap_or_default();
        if status.success() {
            Ok(text(&stdout))
        } else {
            Err(GitError::Failed {
                command,
                detail: text(&stderr),
            })
        }
    }

    /// `git fetch` with `args` in `repo`, bounded by [`FETCH_TIMEOUT`]. Callers ignore a failure
    /// (offline, a hanging network) or read the facts they already have.
    pub(crate) fn fetch<S: AsRef<OsStr>>(&self, repo: &str, args: &[S]) -> Result<(), GitError> {
        self.fetch_within(repo, args, FETCH_TIMEOUT)
    }

    /// [`Self::fetch`] with another limit (tests).
    pub(crate) fn fetch_within<S: AsRef<OsStr>>(
        &self,
        repo: &str,
        args: &[S],
        limit: Duration,
    ) -> Result<(), GitError> {
        let mut full: Vec<&OsStr> = vec![OsStr::new("fetch")];
        full.extend(args.iter().map(AsRef::as_ref));
        self.run_within(repo, &full, limit).map(drop)
    }

    /// Runs git in `repo` and returns its trimmed stdout, or why it failed.
    pub(crate) fn run<S: AsRef<OsStr>>(&self, repo: &str, args: &[S]) -> Result<String, GitError> {
        let out = self.output(repo, args)?;
        if out.status.success() {
            return Ok(text(&out.stdout));
        }
        let command = args
            .iter()
            .map(|a| a.as_ref().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join(" ");
        Err(GitError::Failed {
            command,
            detail: text(&out.stderr),
        })
    }

    /// Whether the command succeeds; a command that fails is `false`, not an error.
    pub(crate) fn succeeds<S: AsRef<OsStr>>(
        &self,
        repo: &str,
        args: &[S],
    ) -> Result<bool, GitError> {
        match self.run(repo, args) {
            Ok(_) => Ok(true),
            Err(GitError::Failed { .. }) => Ok(false),
            Err(e) => Err(e),
        }
    }
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).trim().to_owned()
}

#[cfg(test)]
mod tests;
