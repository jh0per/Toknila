//! A release run: the project's release steps on the base branch, one at a time, with `git` and
//! the project's own commands. The steps run in a throwaway worktree of the base branch, so the
//! person's checkout is never touched; only the last step that changes the repository moves the
//! base branch (a bump commit) and creates the tag. Every step is reported to a journal sink as
//! it goes (see `store::release_runs`): what lasts is the journal, not this process. The run
//! holds its project's merge line (the caller takes the turn before it starts), and its Check
//! waits for a turn in the machine's check line like any check.

use std::path::PathBuf;
use std::time::Duration;

use toknila_app::checks::CheckLine;
use toknila_app::{CheckSpec, Checker};
use toknila_core::release_run::{ReleaseStep, artifact_command, bump_command};
use toknila_core::{ProjectId, ReleaseId};

use crate::checker::ShellChecker;
use crate::git::Git;
use crate::shipyard::remote::Remote;
use crate::store::release_runs::{Entry, tail};

mod publish;

/// How long a push may take.
const PUSH_LIMIT: Duration = Duration::from_secs(300);

/// How long the Build step may take: a notarization alone can take many minutes.
const BUILD_LIMIT: Duration = Duration::from_secs(4 * 60 * 60);

/// Where a run reports its steps; an error stops the run (the journal could not be written).
pub type Journal<'a> = &'a (dyn Fn(Entry) -> Result<(), String> + Sync);

/// Everything a run needs, from the project and the milestone.
#[derive(Debug, Clone)]
pub struct Plan {
    pub release: ReleaseId,
    /// The project whose merge line the run holds.
    pub project: ProjectId,
    pub repo: String,
    /// The branch the release is made on.
    pub base: String,
    pub version: String,
    pub tag: String,
    pub check: Option<String>,
    pub bump: Option<String>,
    /// The command of the Build step, with `{version}` and `{tag}` still to be filled in.
    pub artifact: Option<String>,
    /// Where the Push step sends the base and the tag, as the shipyard read it.
    pub remote: Option<Remote>,
    /// Every step the project's release has.
    pub steps: Vec<ReleaseStep>,
    /// The steps this run takes: all of them, or what is left after an interruption.
    pub todo: Vec<ReleaseStep>,
    /// This run goes on after an interruption.
    pub resuming: bool,
    /// A folder the run may make its worktree in.
    pub scratch: PathBuf,
}

/// The repository's tags, newest version first.
pub fn tags(repo: &str) -> Vec<String> {
    Git::new("git")
        .run(repo, &["tag", "--list", "--sort=-v:refname"])
        .map(|out| out.lines().map(str::to_owned).collect())
        .unwrap_or_default()
}

/// Runs the plan's steps in order and stops at the first failure, which is named in the error.
/// The Check step waits for its turn in `line`, the machine's check line. Blocking: call it
/// from a blocking task.
pub fn execute(plan: &Plan, journal: Journal<'_>, line: &CheckLine) -> Result<(), String> {
    let Some(first) = plan.todo.first().copied() else {
        return Ok(());
    };
    let git = Git::new("git");
    let dir = plan.scratch.join(format!("release-{}", plan.release));
    let tip = match prepare(&git, plan, &dir) {
        Ok(tip) => tip,
        Err(e) => return stop(journal, first, &e),
    };
    let ctx = Ctx {
        plan,
        git: &git,
        dir: dir.to_string_lossy().into_owned(),
        tip,
        line,
    };
    let mut result = Ok(());
    for step in &plan.todo {
        if let Err(e) = journal(Entry::StepStarted { step: *step }) {
            result = Err(e);
            break;
        }
        match ctx.run(*step) {
            Ok(output) => {
                let entry = Entry::StepPassed {
                    step: *step,
                    output: tail(output),
                };
                if let Err(e) = journal(entry) {
                    result = Err(e);
                    break;
                }
            }
            Err(e) => {
                result = stop(journal, *step, &e);
                break;
            }
        }
    }
    let _ = git.run(&plan.repo, &["worktree", "remove", "--force", &ctx.dir]);
    let _ = git.run(&plan.repo, &["worktree", "prune"]);
    result
}

/// Journals the failure of `step` and returns the message that names it.
fn stop(journal: Journal<'_>, step: ReleaseStep, detail: &str) -> Result<(), String> {
    let entry = Entry::StepFailed {
        step,
        output: tail(detail.to_owned()),
    };
    journal(entry)?;
    Err(format!("{} failed: {}", step.label(), first_line(detail)))
}

fn first_line(text: &str) -> &str {
    text.lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("no output")
}

/// A detached worktree of the base branch's tip; returns the tip.
fn prepare(git: &Git, plan: &Plan, dir: &std::path::Path) -> Result<String, String> {
    let tip = git
        .run(
            &plan.repo,
            &[
                "rev-parse",
                "--verify",
                &format!("refs/heads/{}^{{commit}}", plan.base),
            ],
        )
        .map_err(|_| format!("the base branch {} does not exist", plan.base))?;
    std::fs::create_dir_all(&plan.scratch)
        .map_err(|e| format!("cannot make {}: {e}", plan.scratch.display()))?;
    // A worktree an interrupted run left behind is in the way.
    if dir.exists() {
        let _ = git.run(
            &plan.repo,
            &["worktree", "remove", "--force", &dir.to_string_lossy()],
        );
        let _ = std::fs::remove_dir_all(dir);
    }
    let _ = git.run(&plan.repo, &["worktree", "prune"]);
    git.run(
        &plan.repo,
        &["worktree", "add", "--detach", &dir.to_string_lossy(), &tip],
    )
    .map_err(|e| format!("cannot check out {}: {e}", plan.base))?;
    Ok(tip)
}

struct Ctx<'a> {
    plan: &'a Plan,
    git: &'a Git,
    /// The throwaway worktree.
    dir: String,
    /// Where the base branch was when the run began.
    tip: String,
    /// The machine's check line.
    line: &'a CheckLine,
}

impl Ctx<'_> {
    fn run(&self, step: ReleaseStep) -> Result<String, String> {
        match step {
            ReleaseStep::Check => {
                // Only `max_checks` at a time on the machine: wait for the turn, hold it until done.
                let _turn = self.line.enter_release(self.plan.release);
                self.shell(self.plan.check.as_deref().unwrap_or_default())
            }
            ReleaseStep::Bump => self.bump(),
            ReleaseStep::Tag => self.tag(),
            ReleaseStep::Push => self.push(),
            ReleaseStep::Build => {
                // A build loads the machine: it takes a turn in the check line, but none in the
                // merge line (the tag is made, cards may merge meanwhile).
                let _turn = self.line.enter_release(self.plan.release);
                self.build()
            }
        }
    }

    /// The artifact command in the worktree, moved to the tagged commit first: the base branch
    /// may have moved on since (cards merge while a build runs, and a resumed run starts from
    /// the base's tip).
    fn build(&self) -> Result<String, String> {
        let plan = self.plan;
        let template = plan.artifact.as_deref().unwrap_or_default();
        let commit = format!("refs/tags/{}^{{commit}}", plan.tag);
        self.git
            .run(&self.dir, &["checkout", "--quiet", "--detach", &commit])
            .map_err(|e| format!("cannot check out {}: {e}", plan.tag))?;
        let command = artifact_command(template, &plan.version, &plan.tag);
        self.shell_within(&command, BUILD_LIMIT)
    }

    fn shell(&self, command: &str) -> Result<String, String> {
        self.shell_within(command, crate::checker::CHECK_TIMEOUT)
    }

    fn shell_within(&self, command: &str, limit: Duration) -> Result<String, String> {
        let spec = CheckSpec {
            command: command.to_owned(),
            dir: self.dir.clone(),
        };
        match ShellChecker::new(limit).run(&spec) {
            Ok(r) if r.success => Ok(format!("$ {command}\n{}", r.output)),
            Ok(r) if r.timed_out => Err(format!("$ {command}\n{}\ntimed out", r.output)),
            Ok(r) => Err(format!(
                "$ {command}\n{}\nexit code {}",
                r.output,
                r.code
                    .map_or_else(|| "unknown".to_owned(), |c| c.to_string())
            )),
            Err(e) => Err(format!("$ {command}\n{e}")),
        }
    }

    /// `-c` options that give a machine without a git identity one.
    fn identity(&self) -> Vec<&'static str> {
        if self
            .git
            .succeeds(&self.dir, &["var", "GIT_COMMITTER_IDENT"])
            .unwrap_or(false)
        {
            Vec::new()
        } else {
            vec![
                "-c",
                "user.name=Toknila",
                "-c",
                "user.email=toknila@localhost",
            ]
        }
    }

    fn bump(&self) -> Result<String, String> {
        let template = self.plan.bump.as_deref().unwrap_or_default();
        let mut out = self.shell(&bump_command(template, &self.plan.version))?;
        let changed = self
            .git
            .run(&self.dir, &["status", "--porcelain"])
            .map_err(|e| format!("{out}\n{e}"))?;
        if changed.is_empty() {
            out.push_str("\nThe command changed no file; nothing to commit.");
            return Ok(out);
        }
        let message = format!("Release {}", self.plan.version);
        let mut commit = self.identity();
        commit.extend(["commit", "--all", "--message", &message]);
        self.git
            .run(&self.dir, &["add", "--all"])
            .and_then(|_| self.git.run(&self.dir, &commit))
            .map_err(|e| format!("{out}\n{e}"))?;
        out.push_str(&format!("\nCommitted: {message}"));
        Ok(out)
    }
}
