//! The merger over GitHub: a card's ship branch is pushed to `origin`, opened as a pull request
//! with `gh pr create` (or the pull request that already exists for the branch is reused), its
//! checks are watched, and `gh pr merge` merges it once every check passed.
//!
//! One call is one step. The first (`PrSpec::watching` is `None`) pushes and opens; later ones
//! only look. Either way it ends with a [`MergeOutcome`] the machine understands: the checks are
//! still running (`PrOpen`), failed, the pull request was closed or conflicts, or it merged and
//! `origin` was fetched so the next ships start from the new base. Whether the checks are good
//! enough is core's decision ([`toknila_core::pr::judge`]); this file only reads what `gh` says.

use serde::Deserialize;
use toknila_app::{MergeError, MergeOutcome, Merger};
use toknila_core::pr::{ChecksVerdict, PrSpec, judge};
use toknila_core::{CheckRun, CheckState, MergeSpec, PullRequest};

use crate::gh::{Gh, GhError};
use crate::git::Git;
use crate::merger::GitMerger;

/// What `gh pr view --json` says about a pull request.
#[derive(Debug, Deserialize)]
struct View {
    number: u64,
    url: String,
    state: String,
    #[serde(default)]
    mergeable: String,
}

/// One entry of `gh pr checks --json name,bucket`.
#[derive(Debug, Deserialize)]
struct Bucket {
    name: String,
    bucket: String,
}

/// Merges through GitHub pull requests with the `git` and `gh` on the path.
#[derive(Debug, Clone)]
pub struct GithubMerger {
    git: Git,
    gh: Gh,
}

impl Default for GithubMerger {
    fn default() -> Self {
        Self::new()
    }
}

impl GithubMerger {
    pub fn new() -> Self {
        Self {
            git: Git::new("git"),
            gh: Gh::new("gh"),
        }
    }

    /// Runs every command with `name` set to `value`; tests put a fake `gh` first on `PATH`.
    pub fn with_env(mut self, name: &str, value: &str) -> Self {
        self.gh = self.gh.with_env(name, value);
        self
    }

    /// Pushes the ship's branch to `origin` (the person's own git credentials).
    fn push(&self, spec: &MergeSpec) -> Result<(), MergeError> {
        let refspec = format!("refs/heads/{0}:refs/heads/{0}", spec.branch);
        self.git
            .run(&spec.repo, &["push", "origin", &refspec])
            .map(drop)
            .map_err(Into::into)
    }

    /// The pull request `selector` (a branch, a number or a URL) names, if there is one.
    fn view(&self, repo: &str, selector: &str) -> Result<Option<View>, MergeError> {
        let args = [
            "pr",
            "view",
            selector,
            "--json",
            "number,url,state,mergeable",
        ];
        match self.gh.run(repo, &args) {
            Ok(json) => Ok(Some(parse(&json, "pr view")?)),
            Err(GhError::Failed { detail, .. }) if detail.contains("no pull requests found") => {
                Ok(None)
            }
            Err(e) => Err(e.into()),
        }
    }

    /// The branch's open pull request, or a new one.
    fn open(&self, spec: &MergeSpec, pr: &PrSpec) -> Result<View, MergeError> {
        self.push(spec)?;
        if let Some(found) = self.view(&spec.repo, &spec.branch)?
            && found.state != "CLOSED"
        {
            return Ok(found);
        }
        let create = [
            "pr",
            "create",
            "--head",
            &spec.branch,
            "--base",
            &spec.base,
            "--title",
            &pr.title,
            "--body",
            &pr.body,
        ];
        // The last line `gh pr create` prints is the new pull request's URL.
        let out = self.gh.run(&spec.repo, &create)?;
        let url = out.lines().last().unwrap_or_default().trim().to_owned();
        self.view(&spec.repo, &url)?.ok_or_else(|| MergeError::Gh {
            command: "pr create".to_owned(),
            detail: format!("the new pull request {url} cannot be found"),
        })
    }

    /// The checks as `gh` reports them; none yet is an empty list.
    fn checks(&self, repo: &str, number: u64) -> Result<Vec<CheckRun>, MergeError> {
        let number = number.to_string();
        let out = self
            .gh
            .output(repo, &["pr", "checks", &number, "--json", "name,bucket"])?;
        // `gh pr checks` exits non-zero while checks are pending or failed, and still prints them.
        let stdout = String::from_utf8_lossy(&out.stdout);
        if stdout.trim().is_empty() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            return if out.status.success() || stderr.contains("no checks reported") {
                Ok(Vec::new())
            } else {
                Err(MergeError::Gh {
                    command: "pr checks".to_owned(),
                    detail: stderr.trim().to_owned(),
                })
            };
        }
        let buckets: Vec<Bucket> = parse(&stdout, "pr checks")?;
        Ok(buckets
            .into_iter()
            .map(|b| CheckRun {
                state: match b.bucket.as_str() {
                    "pass" | "skipping" => CheckState::Passed,
                    "fail" | "cancel" => CheckState::Failed,
                    _ => CheckState::Pending,
                },
                name: b.name,
            })
            .collect())
    }

    /// One look at the pull request, and the merge when it is ready.
    fn step(&self, spec: &MergeSpec, pr: &PrSpec, view: View) -> Result<MergeOutcome, MergeError> {
        let repo = spec.repo.as_str();
        let checks = self.checks(repo, view.number)?;
        let pull = PullRequest {
            number: view.number,
            url: view.url,
            checks,
        };
        match view.state.as_str() {
            "MERGED" => return self.landed(spec, pull),
            "CLOSED" => return Ok(MergeOutcome::PrClosed { url: pull.url }),
            _ => {}
        }
        if view.mergeable == "CONFLICTING" {
            return Ok(MergeOutcome::PrConflict { url: pull.url });
        }
        match judge(&pull.checks) {
            ChecksVerdict::Pending => Ok(MergeOutcome::PrOpen { pr: pull }),
            ChecksVerdict::Failed(failed) => Ok(MergeOutcome::CiFailed { pr: pull, failed }),
            ChecksVerdict::Passed => {
                let number = pull.number.to_string();
                match self
                    .gh
                    .run(repo, &["pr", "merge", &number, pr.method.flag()])
                {
                    Ok(_) => self.landed(spec, pull),
                    Err(GhError::Failed { detail, .. }) if conflicts(&detail) => {
                        Ok(MergeOutcome::PrConflict { url: pull.url })
                    }
                    Err(e) => Err(e.into()),
                }
            }
        }
    }

    /// The pull request is merged: fetch `origin`, so ships made from now on start from it.
    fn landed(&self, spec: &MergeSpec, pr: PullRequest) -> Result<MergeOutcome, MergeError> {
        self.git.run(&spec.repo, &["fetch", "origin"])?;
        Ok(MergeOutcome::PrMerged { pr })
    }
}

/// Whether `gh pr merge` refused because the branch no longer merges cleanly.
fn conflicts(detail: &str) -> bool {
    let detail = detail.to_lowercase();
    detail.contains("conflict") || detail.contains("not mergeable")
}

fn parse<T: for<'de> Deserialize<'de>>(json: &str, command: &str) -> Result<T, MergeError> {
    serde_json::from_str(json).map_err(|e| MergeError::Gh {
        command: command.to_owned(),
        detail: format!("cannot read what gh printed: {e}"),
    })
}

impl Merger for GithubMerger {
    fn merge(&self, spec: &MergeSpec) -> Result<MergeOutcome, MergeError> {
        let Some(pr) = &spec.pr else {
            return Err(MergeError::Io(
                "a pull-request merge needs a pull request spec".to_owned(),
            ));
        };
        let view = match &pr.watching {
            Some(open) => self.view(&spec.repo, &open.number.to_string())?,
            None => Some(self.open(spec, pr)?),
        };
        match view {
            Some(view) => self.step(spec, pr, view),
            None => Err(MergeError::Gh {
                command: "pr view".to_owned(),
                detail: "the pull request cannot be found any more".to_owned(),
            }),
        }
    }
}

/// The merger a daemon uses: a local `git` merge, or a pull request when the spec asks for one.
#[derive(Debug, Default)]
pub struct Mergers {
    local: GitMerger,
    github: GithubMerger,
}

impl Mergers {
    /// Local merges with `git`, pull requests with `git` and `gh`, both from the path.
    pub fn new() -> Self {
        Self::default()
    }

    /// Like [`Self::new`] with `github` as the pull-request merger.
    pub fn with_github(github: GithubMerger) -> Self {
        Self {
            github,
            ..Self::default()
        }
    }
}

impl Merger for Mergers {
    fn merge(&self, spec: &MergeSpec) -> Result<MergeOutcome, MergeError> {
        if spec.pr.is_some() {
            self.github.merge(spec)
        } else {
            self.local.merge(spec)
        }
    }
}
