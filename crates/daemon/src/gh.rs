//! Running the GitHub CLI (`gh`) for the pull-request merger. No shell: the program with an
//! argument list. A missing `gh` and a missing login are their own errors, so the card can say
//! how to fix them.

use std::ffi::OsStr;
use std::io::ErrorKind;
use std::process::{Command, Output};

use toknila_app::MergeError;

/// Why a `gh` command did not give a result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum GhError {
    /// The program is not installed or not on the path.
    Missing,
    /// `gh` has no GitHub login.
    NotLoggedIn,
    /// The process could not be run.
    Io(String),
    /// The command ran and exited with a failure.
    Failed { command: String, detail: String },
}

impl From<GhError> for MergeError {
    fn from(e: GhError) -> Self {
        match e {
            GhError::Missing => MergeError::GhMissing,
            GhError::NotLoggedIn => MergeError::GhNotLoggedIn,
            GhError::Io(text) => MergeError::Io(text),
            GhError::Failed { command, detail } => MergeError::Gh { command, detail },
        }
    }
}

/// A `gh` executable.
#[derive(Debug, Clone)]
pub(crate) struct Gh {
    program: String,
    /// Environment set for every command (tests put a fake `gh` first on `PATH`).
    envs: Vec<(String, String)>,
}

impl Gh {
    pub(crate) fn new(program: &str) -> Self {
        Self {
            program: program.to_owned(),
            envs: Vec::new(),
        }
    }

    /// Sets `name` to `value` for every command.
    pub(crate) fn with_env(mut self, name: &str, value: &str) -> Self {
        self.envs.push((name.to_owned(), value.to_owned()));
        self
    }

    /// Runs `gh` in `dir` and returns whatever it printed, whether it succeeded or not; only a
    /// missing program or a missing login are errors.
    pub(crate) fn output<S: AsRef<OsStr>>(&self, dir: &str, args: &[S]) -> Result<Output, GhError> {
        let out = Command::new(&self.program)
            .current_dir(dir)
            .args(args)
            .envs(self.envs.iter().map(|(k, v)| (k, v)))
            .env("GH_PROMPT_DISABLED", "1")
            .env("NO_COLOR", "1")
            .output()
            .map_err(|e| match e.kind() {
                ErrorKind::NotFound => GhError::Missing,
                _ => GhError::Io(format!("cannot run {}: {e}", self.program)),
            })?;
        if !out.status.success() && text(&out.stderr).contains("gh auth login") {
            return Err(GhError::NotLoggedIn);
        }
        Ok(out)
    }

    /// Runs `gh` in `dir` and returns its trimmed stdout, or why it failed.
    pub(crate) fn run<S: AsRef<OsStr>>(&self, dir: &str, args: &[S]) -> Result<String, GhError> {
        let out = self.output(dir, args)?;
        if out.status.success() {
            return Ok(text(&out.stdout));
        }
        let command = args
            .iter()
            .map(|a| a.as_ref().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join(" ");
        Err(GhError::Failed {
            command,
            detail: text(&out.stderr),
        })
    }
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).trim().to_owned()
}
