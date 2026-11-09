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

    /// The pull request with this number or URL. One that cannot be found is a failure here: the
    /// caller got the number from `gh` itself.
    fn view(&self, repo: &str, selector: &str) -> Result<View, MergeError> {
        let args = [
            "pr",
            "view",
            selector,
            "--json",
            "number,url,state,mergeable",
        ];
        let json = self.gh.run(repo, &args)?;
        parse(&json, "pr view")
    }

    /// The pull request of `branch`, if it has one: the open one, else the newest. `gh pr list`
    /// answers a branch without a pull request with an empty list and exit code 0, so "none" is
    /// read from the JSON and not from a message.
    fn find(&self, repo: &str, branch: &str) -> Result<Option<View>, MergeError> {
        let args = [
            "pr",
            "list",
            "--head",
            branch,
            "--state",
            "all",
            "--json",
            "number,url,state,mergeable",
        ];
        let json = self.gh.run(repo, &args)?;
        let mut all: Vec<View> = parse(&json, "pr list")?;
        all.sort_by_key(|v| (v.state == "OPEN", v.number));
        Ok(all.pop())
    }

    /// The branch's open pull request, or a new one.
    fn open(&self, spec: &MergeSpec, pr: &PrSpec) -> Result<View, MergeError> {
        self.push(spec)?;
        if let Some(found) = self.find(&spec.repo, &spec.branch)?
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
        self.view(&spec.repo, &url)
    }

    /// The checks as `gh` reports them; none yet is an empty list.
    ///
    /// `gh pr checks` exits non-zero while checks are pending or failed, and still prints them
    /// as JSON. When it prints no JSON at all it either found no check or failed; which one is
    /// told by asking `gh pr view --json statusCheckRollup` (JSON, exit code 0 when it works),
    /// never by reading the message.
    fn checks(&self, repo: &str, number: u64) -> Result<Vec<CheckRun>, MergeError> {
        let number = number.to_string();
        let out = self
            .gh
            .output(repo, &["pr", "checks", &number, "--json", "name,bucket"])?;
        let stdout = String::from_utf8_lossy(&out.stdout);
        if stdout.trim().is_empty() {
            return if self.rollup_is_empty(repo, &number)? {
                Ok(Vec::new())
            } else {
                Err(MergeError::Gh {
                    command: "pr checks".to_owned(),
                    detail: format!(
                        "gh exited with {:?} and printed no checks: {}",
                        out.status.code(),
                        String::from_utf8_lossy(&out.stderr).trim()
                    ),
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

    /// Whether the pull request's status rollup is an empty list: no check of any kind reported.
    fn rollup_is_empty(&self, repo: &str, number: &str) -> Result<bool, MergeError> {
        #[derive(Deserialize)]
        struct Rollup {
            #[serde(rename = "statusCheckRollup")]
            checks: Vec<serde_json::Value>,
        }
        let json = self
            .gh
            .run(repo, &["pr", "view", number, "--json", "statusCheckRollup"])?;
        let rollup: Rollup = parse(&json, "pr view")?;
        Ok(rollup.checks.is_empty())
    }

    /// One look at the pull request, and the merge when it is ready.
    fn step(&self, spec: &MergeSpec, pr: &PrSpec, view: View) -> Result<MergeOutcome, MergeError> {
        let repo = spec.repo.as_str();
        let checks = self.checks(repo, view.number)?;
        let pull = PullRequest::seen(view.number, view.url, checks);
        match view.state.as_str() {
            "MERGED" => return self.landed(spec, pull),
            "CLOSED" => return Ok(MergeOutcome::PrClosed { url: pull.url }),
            _ => {}
        }
        if view.mergeable == "CONFLICTING" {
            return Ok(MergeOutcome::PrConflict { url: pull.url });
        }
        // A person answered "merge now" to the no-checks question: a pull request that still
        // reports no check is merged as it is.
        let unchecked =
            pull.checks.is_empty() && pr.watching.as_ref().is_some_and(|w| w.merge_unchecked);
        match judge(&pull.checks) {
            ChecksVerdict::Pending if !unchecked => Ok(MergeOutcome::PrOpen { pr: pull }),
            ChecksVerdict::Failed(failed) => Ok(MergeOutcome::CiFailed { pr: pull, failed }),
            ChecksVerdict::Pending | ChecksVerdict::Passed => {
                let number = pull.number.to_string();
                match self
                    .gh
                    .run(repo, &["pr", "merge", &number, pr.method.flag()])
                {
                    Ok(_) => self.landed(spec, pull),
                    // Why `gh pr merge` refused is not read from its message: the pull request
                    // is looked at again, and GitHub says whether it conflicts.
                    Err(refused @ GhError::Failed { .. }) => {
                        if self.view(repo, &number)?.mergeable == "CONFLICTING" {
                            Ok(MergeOutcome::PrConflict { url: pull.url })
                        } else {
                            Err(refused.into())
                        }
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
            None => self.open(spec, pr)?,
        };
        self.step(spec, pr, view)
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
