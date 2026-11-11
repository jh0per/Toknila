//! A release run: the project's release steps on the base branch, one at a time, with `git` and
//! the project's own commands. The steps run in a throwaway worktree of the base branch, so the
//! person's checkout is never touched; only the last step that changes the repository moves the
//! base branch (a bump commit) and creates the tag. The state of the last run of each milestone
//! is kept in memory for the page (`GET /releases/{release}/run`); what lasts is the milestone's
//! state and tag in the database.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use toknila_api_types::{ReleaseRun, RunState, RunStep, StepState};
use toknila_app::{CheckSpec, Checker};
use toknila_core::ReleaseId;
use toknila_core::release_run::{ReleaseStep, bump_command};

use crate::checker::ShellChecker;
use crate::git::Git;
use crate::merger::GitMerger;

/// How long a push may take.
const PUSH_LIMIT: Duration = Duration::from_secs(300);
/// The most bytes of a step's output kept for the page (the end of it).
const KEEP: usize = 64 * 1024;

fn runs() -> &'static Mutex<HashMap<ReleaseId, ReleaseRun>> {
    static RUNS: OnceLock<Mutex<HashMap<ReleaseId, ReleaseRun>>> = OnceLock::new();
    RUNS.get_or_init(Mutex::default)
}

fn with_runs<T>(f: impl FnOnce(&mut HashMap<ReleaseId, ReleaseRun>) -> T) -> T {
    let mut guard = runs()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    f(&mut guard)
}

/// The last release run of a milestone, if this daemon ran one.
pub fn run_of(release: ReleaseId) -> Option<ReleaseRun> {
    with_runs(|m| m.get(&release).cloned())
}

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
    pub steps: Vec<ReleaseStep>,
    /// A folder the run may make its worktree in.
    pub scratch: PathBuf,
}

/// Registers a new run with every step pending. `false`: the milestone's last run is still going.
pub fn begin(plan: &Plan) -> bool {
    with_runs(|m| {
        if m.get(&plan.release)
            .is_some_and(|r| r.state == RunState::Running)
        {
            return false;
        }
        let steps = plan
            .steps
            .iter()
            .map(|s| RunStep {
                step: *s,
                label: s.label().to_owned(),
                state: StepState::Pending,
                output: String::new(),
            })
            .collect();
        m.insert(
            plan.release,
            ReleaseRun {
                release: plan.release.to_string(),
                version: plan.version.clone(),
                tag: plan.tag.clone(),
                state: RunState::Running,
                steps,
                error: None,
            },
        );
        true
    })
}

/// Marks the run done: every step passed and the milestone is released.
pub fn done(release: ReleaseId) {
    with_runs(|m| {
        if let Some(r) = m.get_mut(&release) {
            r.state = RunState::Done;
        }
    });
}

/// Marks the run failed with `why`, naming no step (the milestone could not be saved).
pub fn failed(release: ReleaseId, why: String) {
    with_runs(|m| {
        if let Some(r) = m.get_mut(&release) {
            r.state = RunState::Failed;
            r.error = Some(why);
        }
    });
}

fn set_step(release: ReleaseId, step: ReleaseStep, state: StepState, output: Option<String>) {
    with_runs(|m| {
        if let Some(s) = m
            .get_mut(&release)
            .and_then(|r| r.steps.iter_mut().find(|s| s.step == step))
        {
            s.state = state;
            if let Some(out) = output {
                s.output = tail(out);
            }
        }
    });
}

fn tail(mut text: String) -> String {
    if text.len() > KEEP {
        let mut from = text.len() - KEEP;
        while !text.is_char_boundary(from) {
            from += 1;
        }
        text = text[from..].to_owned();
    }
    text
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
pub fn execute(plan: &Plan) -> Result<(), String> {
    let git = Git::new("git");
    let dir = plan.scratch.join(format!("release-{}", plan.release));
    let tip = match prepare(&git, plan, &dir) {
        Ok(tip) => tip,
        Err(e) => return Err(stop(plan, plan.steps[0], &e)),
    };
    let ctx = Ctx {
        plan,
        git: &git,
        dir: dir.to_string_lossy().into_owned(),
        tip,
    };
    let mut result = Ok(());
    for step in &plan.steps {
        set_step(plan.release, *step, StepState::Running, None);
        match ctx.run(*step) {
            Ok(out) => set_step(plan.release, *step, StepState::Done, Some(out)),
            Err(e) => {
                result = Err(stop(plan, *step, &e));
                break;
            }
        }
    }
    let _ = git.run(&plan.repo, &["worktree", "remove", "--force", &ctx.dir]);
    let _ = git.run(&plan.repo, &["worktree", "prune"]);
    result
}

/// Records the failure of `step` and returns the message that names it.
fn stop(plan: &Plan, step: ReleaseStep, detail: &str) -> String {
    let message = format!("{} failed: {}", step.label(), first_line(detail));
    with_runs(|m| {
        if let Some(r) = m.get_mut(&plan.release) {
            r.state = RunState::Failed;
            r.error = Some(message.clone());
            if let Some(s) = r.steps.iter_mut().find(|s| s.step == step) {
                s.state = StepState::Failed;
                s.output = tail(detail.to_owned());
            }
        }
    });
    message
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
