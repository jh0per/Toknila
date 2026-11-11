//! A release run: the project's release steps on the base branch, one at a time, with `git` and
//! the project's own commands. The steps run in a throwaway worktree of the base branch, so the
//! person's checkout is never touched; only the last step that changes the repository moves the
//! base branch (a bump commit) and creates the tag. Every step is reported to a journal sink as
//! it goes (see `store::release_runs`): what lasts is the journal, not this process.

use std::path::PathBuf;
use std::time::Duration;

use toknila_app::{CheckSpec, Checker};
use toknila_core::ReleaseId;
use toknila_core::release_run::{ReleaseStep, bump_command};

use crate::checker::ShellChecker;
use crate::git::Git;
use crate::merger::GitMerger;
use crate::store::release_runs::{Entry, tail};

/// How long a push may take.
const PUSH_LIMIT: Duration = Duration::from_secs(300);

/// Where a run reports its steps; an error stops the run (the journal could not be written).
pub type Journal<'a> = &'a (dyn Fn(Entry) -> Result<(), String> + Sync);

/// Everything a run needs, from the project and the milestone.
#[derive(Debug, Clone)]
pub struct Plan {
    pub release: ReleaseId,
    pub repo: String,
    /// The branch the release is made on.
    pub base: String,
    pub version: String,
    pub tag: String,
    pub check: Option<String>,
    pub bump: Option<String>,
    pub push: bool,
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
/// Blocking: call it from a blocking task.
pub fn execute(plan: &Plan, journal: Journal<'_>) -> Result<(), String> {
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
}

impl Ctx<'_> {
    fn run(&self, step: ReleaseStep) -> Result<String, String> {
        match step {
            ReleaseStep::Check => self.shell(self.plan.check.as_deref().unwrap_or_default()),
            ReleaseStep::Bump => self.bump(),
            ReleaseStep::Tag => self.tag(),
            ReleaseStep::Push => self.push(),
        }
    }

    fn shell(&self, command: &str) -> Result<String, String> {
        let spec = CheckSpec {
            command: command.to_owned(),
            dir: self.dir.clone(),
        };
        match ShellChecker::default().run(&spec) {
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

    fn tag(&self) -> Result<String, String> {
        let plan = self.plan;
        if plan.resuming
            && let Some(done) = self.left_by_interruption()
        {
            return Ok(done);
        }
        let message = format!("Release {}", plan.version);
        let mut args = self.identity();
        args.extend(["tag", "--annotate", "--message", &message, &plan.tag]);
        self.git.run(&self.dir, &args).map_err(|e| e.to_string())?;
        let head = self
            .git
            .run(&self.dir, &["rev-parse", "HEAD"])
            .map_err(|e| e.to_string())?;
        let mut out = format!("Tagged {} at {}", plan.tag, &head[..head.len().min(10)]);
        if head != self.tip {
            let moved = GitMerger::new().advance_base(&plan.repo, &plan.base, &self.tip, &head);
            let undo = |why: String| {
                let _ = self.git.run(&plan.repo, &["tag", "--delete", &plan.tag]);
                why
            };
            match moved {
                Ok(true) => out.push_str(&format!("\n{} now includes the bump commit.", plan.base)),
                Ok(false) => {
                    return Err(undo(format!(
                        "{} moved while the release ran; nothing was tagged. Release again",
                        plan.base
                    )));
                }
                Err(e) => return Err(undo(e.to_string())),
            }
        }
        Ok(out)
    }

    /// A tag an interrupted run left: when it is on the base branch the step is done (`Some`);
    /// when it is not (the run stopped before the branch moved) it is removed and the step is
    /// done afresh.
    fn left_by_interruption(&self) -> Option<String> {
        let plan = self.plan;
        let tag_ref = format!("refs/tags/{}", plan.tag);
        let exists = self
            .git
            .succeeds(&plan.repo, &["rev-parse", "--verify", "--quiet", &tag_ref])
            .unwrap_or(false);
        if !exists {
            return None;
        }
        let commit = format!("{tag_ref}^{{commit}}");
        let base = format!("refs/heads/{}", plan.base);
        let on_base = self
            .git
            .succeeds(&plan.repo, &["merge-base", "--is-ancestor", &commit, &base])
            .unwrap_or(false);
        if on_base {
            return Some(format!("{} was already made on {}", plan.tag, plan.base));
        }
        let _ = self.git.run(&plan.repo, &["tag", "--delete", &plan.tag]);
        None
    }

    fn push(&self) -> Result<String, String> {
        let plan = self.plan;
        let args = [
            "push",
            "--porcelain",
            "origin",
            &format!("refs/heads/{}", plan.base),
            &format!("refs/tags/{}", plan.tag),
        ];
        self.git
            .run_within(&plan.repo, &args, PUSH_LIMIT)
            .map(|out| format!("$ git push origin {} {}\n{out}", plan.base, plan.tag))
            .map_err(|e| e.to_string())
    }
}
