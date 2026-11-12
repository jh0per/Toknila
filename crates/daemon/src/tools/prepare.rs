//! Making a folder ready to be a project's repository: the one fix its check offers.

use std::path::Path;

use toknila_core::repo::{RepoCheck, RepoFix};
use toknila_core::system::Probe;

use super::{Tools, check_repo, git, ok_line};

const NO_IDENTITY: &str = "git does not know who you are, so it cannot make the first commit. \
     Run `git config --global user.name \"Your Name\"` and \
     `git config --global user.email \"you@example.com\"`, then try again.";

/// Why a fix was not made; the folder is as it was, except where the text says otherwise.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct PrepareError(pub String);

fn fail(text: impl Into<String>) -> PrepareError {
    PrepareError(text.into())
}

fn identity_known(tools: &dyn Tools, path: &str) -> bool {
    ["user.name", "user.email"]
        .iter()
        .all(|key| ok_line(git(tools, path, &["config", "--get", key])).is_some())
}

fn why(probe: Probe) -> String {
    match probe {
        Probe::Ran { stderr, stdout, .. } => {
            let text = if stderr.trim().is_empty() {
                stdout
            } else {
                stderr
            };
            text.trim().to_owned()
        }
        Probe::Missing => "git is not installed".to_owned(),
        Probe::TimedOut => "git took too long".to_owned(),
        Probe::Failed(e) => e,
    }
}

fn run_ok(tools: &dyn Tools, path: &str, args: &[&str]) -> Result<(), PrepareError> {
    match git(tools, path, args) {
        Probe::Ran { code: Some(0), .. } => Ok(()),
        other => Err(fail(format!("git {} failed: {}", args[0], why(other)))),
    }
}

fn first_commit(tools: &dyn Tools, path: &str) -> Result<(), PrepareError> {
    run_ok(
        tools,
        path,
        &["commit", "--allow-empty", "--quiet", "-m", "Initial commit"],
    )
}

/// Does `fix` on the folder, if its check offers it right now, and answers the new check.
pub fn prepare_repo(
    tools: &dyn Tools,
    path: &str,
    fix: RepoFix,
) -> Result<RepoCheck, PrepareError> {
    let path = path.trim();
    let before = check_repo(tools, path, "");
    if !before.problems.first().is_some_and(|p| p.fix == Some(fix)) {
        let said = before
            .problems
            .first()
            .map_or("nothing is wrong with it".to_owned(), |p| p.message.clone());
        return Err(fail(format!("The folder cannot be fixed that way: {said}")));
    }
    match fix {
        RepoFix::CreateFolder => std::fs::create_dir_all(Path::new(path))
            .map_err(|e| fail(format!("Cannot create {path}: {e}")))?,
        RepoFix::InitRepository => {
            if !identity_known(tools, path) {
                return Err(fail(NO_IDENTITY));
            }
            let branch = ok_line(git(tools, path, &["config", "--get", "init.defaultBranch"]))
                .unwrap_or_else(|| "main".to_owned());
            let initial = format!("--initial-branch={branch}");
            run_ok(tools, path, &["init", "--quiet", &initial])?;
            first_commit(tools, path)?;
        }
        RepoFix::FirstCommit => {
            if !identity_known(tools, path) {
                return Err(fail(NO_IDENTITY));
            }
            first_commit(tools, path)?;
        }
    }
    Ok(check_repo(tools, path, ""))
}
