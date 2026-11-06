//! The shipyard over `git`: one worktree per ticket under `<home>/ships/<project>/<chain>/<slug>`.
//!
//! No shell: every command is `git` with an argument list through [`std::process::Command`], so
//! paths with spaces or odd characters pass through untouched on every platform. A missing `git`
//! is [`ShipError::GitMissing`], not a panic.

use std::ffi::OsStr;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::Command;

use toknila_app::{ShipError, Shipyard};
use toknila_core::{Ship, ShipSpec};

/// Makes and removes ships with the `git` on the path.
#[derive(Debug, Clone)]
pub struct GitShipyard {
    root: PathBuf,
    program: String,
}

impl GitShipyard {
    /// A shipyard whose worktrees live under `root` (the home's `ships` directory).
    pub fn new(root: PathBuf) -> Self {
        Self::with_program(root, "git")
    }

    /// Like [`Self::new`] with another executable than `git` (tests use a missing one).
    pub fn with_program(root: PathBuf, program: &str) -> Self {
        // A relative home would make the stored path depend on the daemon's working directory.
        let root = std::path::absolute(&root).unwrap_or(root);
        Self {
            root,
            program: program.to_owned(),
        }
    }

    fn worktree_path(&self, spec: &ShipSpec) -> PathBuf {
        self.root
            .join(&spec.project)
            .join(&spec.chain)
            .join(&spec.slug)
    }

    /// Runs git in `repo` and returns its trimmed stdout, or why it failed.
    fn git<I, S>(&self, repo: &str, args: I) -> Result<String, ShipError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let args: Vec<S> = args.into_iter().collect();
        let out = Command::new(&self.program)
            .arg("-C")
            .arg(repo)
            .args(&args)
            .env("GIT_TERMINAL_PROMPT", "0")
            .output()
            .map_err(|e| match e.kind() {
                ErrorKind::NotFound => ShipError::GitMissing,
                _ => ShipError::Io(format!("cannot run {}: {e}", self.program)),
            })?;
        if out.status.success() {
            return Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned());
        }
        let command = args
            .iter()
            .map(|a| a.as_ref().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join(" ");
        Err(ShipError::Git {
            command,
            detail: String::from_utf8_lossy(&out.stderr).trim().to_owned(),
        })
    }

    fn succeeds<I, S>(&self, repo: &str, args: I) -> Result<bool, ShipError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        match self.git(repo, args) {
            Ok(_) => Ok(true),
            Err(ShipError::Git { .. }) => Ok(false),
            Err(e) => Err(e),
        }
    }
}

/// A worktree git made has a `.git` entry in it.
fn is_worktree(path: &Path) -> bool {
    path.join(".git").exists()
}

fn is_empty_dir(path: &Path) -> bool {
    std::fs::read_dir(path).is_ok_and(|mut d| d.next().is_none())
}

fn path_text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

impl Shipyard for GitShipyard {
    fn prepare(&self, spec: &ShipSpec) -> Result<String, ShipError> {
        let repo = spec.repo.as_str();
        if !self.succeeds(repo, ["rev-parse", "--git-dir"])? {
            return Err(ShipError::NotARepository(spec.repo.clone()));
        }
        let path = self.worktree_path(spec);
        if is_worktree(&path) {
            return Ok(path_text(&path));
        }
        if path.exists() && !is_empty_dir(&path) {
            return Err(ShipError::PathTaken(path_text(&path)));
        }
        // A worktree deleted by hand stays registered and would block the new one.
        self.git(repo, ["worktree", "prune"])?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| ShipError::Io(format!("cannot create {}: {e}", parent.display())))?;
        }
        let branch_ref = format!("refs/heads/{}", spec.branch);
        let branch_exists =
            self.succeeds(repo, ["rev-parse", "--verify", "--quiet", &branch_ref])?;
        let mut args = vec!["worktree".to_owned(), "add".to_owned()];
        if branch_exists {
            args.extend([path_text(&path), spec.branch.clone()]);
        } else {
            let base = format!("{}^{{commit}}", spec.base);
            let base_ok = !spec.base.starts_with('-')
                && self.succeeds(repo, ["rev-parse", "--verify", "--quiet", &base])?;
            if !base_ok {
                return Err(ShipError::BaseBranchMissing(spec.base.clone()));
            }
            args.extend([
                "-b".to_owned(),
                spec.branch.clone(),
                path_text(&path),
                spec.base.clone(),
            ]);
        }
        self.git(repo, &args)?;
        Ok(path_text(&path))
    }

    fn remove(&self, repo: &str, ship: &Ship) -> Result<(), ShipError> {
        let path = Path::new(&ship.path);
        if path.exists() {
            self.git(repo, ["worktree", "remove", "--force", &ship.path])?;
        }
        self.git(repo, ["worktree", "prune"])?;
        // Tidy the chain and project folders once they hold no more ships; never an error.
        for dir in path.ancestors().skip(1).take(2) {
            if std::fs::remove_dir(dir).is_err() {
                break;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
