//! Running `git` for the shipyard and the merger. No shell: every command is the program with an
//! argument list through [`std::process::Command`], so paths with spaces or odd characters pass
//! through untouched on every platform. A missing `git` is [`GitError::Missing`], not a panic.

use std::ffi::OsStr;
use std::io::ErrorKind;
use std::process::{Command, Output};

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
