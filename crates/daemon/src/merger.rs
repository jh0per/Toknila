//! The merger over `git`: a card's ship branch goes into the base branch with a merge commit
//! (two parents), and the person's checkout is never disturbed.
//!
//! The merge is computed without a working tree: `git merge-tree --write-tree` (git 2.38 or
//! newer) gives the merged tree or the conflicting files, `git commit-tree` makes the commit.
//! Only then does the base branch move, and it moves safely:
//!
//! - The base is checked out somewhere (`git worktree list --porcelain`): updating its ref alone
//!   would leave that checkout stale. If it has local changes to tracked files the merge stops
//!   with [`MergeOutcome::BaseCheckoutDirty`] and nothing changes; otherwise
//!   `git -C <checkout> merge --ff-only <new>` moves the branch and the files together, and git
//!   itself refuses (changing nothing) if that would overwrite something.
//! - The base is checked out nowhere: `git update-ref <base> <new> <old>`, a compare-and-swap.
//!
//! Either way the update only succeeds if the base is still where the merge started. If it moved
//! meanwhile, the merge is computed again from the new tip (up to [`ATTEMPTS`] times), then fails
//! with [`MergeError::BaseMoved`]; unblocking the card tries again. The ship's branch only has to
//! exist: where the ship's worktree is, and what is in it, does not matter.

use toknila_app::{MergeError, MergeOutcome, Merger};
use toknila_core::MergeSpec;
use toknila_core::system::{MIN_GIT, git_version};

use crate::git::{Git, GitError};

/// How many times a merge starts over when the base branch moved under it.
const ATTEMPTS: u32 = 3;

/// Merges with the `git` on the path.
pub struct GitMerger {
    git: Git,
    /// Runs just before the base is updated, with the attempt number (tests move the base here).
    #[cfg(test)]
    hook: Option<Box<dyn Fn(u32) + Send + Sync>>,
}

impl std::fmt::Debug for GitMerger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GitMerger")
            .field("git", &self.git)
            .finish_non_exhaustive()
    }
}

impl Default for GitMerger {
    fn default() -> Self {
        Self::new()
    }
}

/// What one try at the merge came to.
enum Step {
    Done(MergeOutcome),
    /// The base branch moved; start over.
    Moved,
}

impl GitMerger {
    pub fn new() -> Self {
        Self::with_program("git")
    }

    /// Like [`Self::new`] with another executable than `git` (tests use a missing one).
    pub fn with_program(program: &str) -> Self {
        Self {
            git: Git::new(program),
            #[cfg(test)]
            hook: None,
        }
    }

    #[cfg(test)]
    fn with_hook(hook: Box<dyn Fn(u32) + Send + Sync>) -> Self {
        Self {
            hook: Some(hook),
            ..Self::new()
        }
    }

    fn before_update(&self, _attempt: u32) {
        #[cfg(test)]
        if let Some(hook) = &self.hook {
            hook(_attempt);
        }
    }

    /// The commit a ref points to, or `None` when there is no such ref.
    fn resolve(&self, repo: &str, refname: &str) -> Result<Option<String>, MergeError> {
        let commit = format!("{refname}^{{commit}}");
        match self
            .git
            .run(repo, &["rev-parse", "--verify", "--quiet", &commit])
        {
            Ok(id) => Ok(Some(id)),
            Err(GitError::Failed { .. }) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// The merged tree of `old` and `theirs`, or the files that conflict.
    fn merge_tree(
        &self,
        repo: &str,
        old: &str,
        theirs: &str,
    ) -> Result<Result<String, Vec<String>>, MergeError> {
        let args = [
            "merge-tree",
            "--write-tree",
            "--name-only",
            "-z",
            old,
            theirs,
        ];
        let out = self.git.output(repo, &args)?;
        let stdout = String::from_utf8_lossy(&out.stdout);
        // With -z: the tree, then (on a conflict) one conflicting file per field up to an empty one.
        let mut fields = stdout.split('\0');
        let tree = fields.next().unwrap_or_default().trim().to_owned();
        match out.status.code() {
            Some(0) if !tree.is_empty() => Ok(Ok(tree)),
            Some(1) => {
                let mut files: Vec<String> = Vec::new();
                for file in fields.take_while(|f| !f.is_empty()) {
                    if !files.iter().any(|known| known == file) {
                        files.push(file.to_owned());
                    }
                }
                Ok(Err(files))
            }
            _ => Err(MergeError::Git {
                command: args.join(" "),
                detail: String::from_utf8_lossy(&out.stderr).trim().to_owned(),
            }),
        }
    }

    /// The merge commit: `tree` with parents `old` and `theirs`. A machine without a git identity
    /// still gets one.
    fn commit_tree(
        &self,
        spec: &MergeSpec,
        tree: &str,
        old: &str,
        theirs: &str,
    ) -> Result<String, MergeError> {
        let repo = spec.repo.as_str();
        let mut args = Vec::new();
        if !self.git.succeeds(repo, &["var", "GIT_COMMITTER_IDENT"])? {
            args.extend([
                "-c",
                "user.name=Toknila",
                "-c",
                "user.email=toknila@localhost",
            ]);
        }
        args.extend([
            "commit-tree",
            tree,
            "-p",
            old,
            "-p",
            theirs,
            "-m",
            &spec.message,
        ]);
        Ok(self.git.run(repo, &args)?)
    }

    /// Moves the base branch from `old` to `new` without disturbing anyone's checkout.
    fn update_base(&self, spec: &MergeSpec, old: &str, new: &str) -> Result<Step, MergeError> {
        let repo = spec.repo.as_str();
        let base_ref = format!("refs/heads/{}", spec.base);
        let listing = self.git.run(repo, &["worktree", "list", "--porcelain"])?;
        let Some(checkout) = checkout_of(&listing, &base_ref) else {
            let swapped = self
                .git
                .succeeds(repo, &["update-ref", &base_ref, new, old])?;
            return Ok(if swapped { merged(new) } else { Step::Moved });
        };
        let changes = self.git.run(
            &checkout,
            &["status", "--porcelain", "--untracked-files=no"],
        )?;
        if !changes.is_empty() {
            return Ok(Step::Done(MergeOutcome::BaseCheckoutDirty {
                path: checkout,
            }));
        }
        if self.git.succeeds(&checkout, &["merge", "--ff-only", new])? {
            return Ok(merged(new));
        }
        // Not a fast-forward because the base moved, or git refused to touch the files (for
        // instance an untracked file is in the way); either way nothing changed.
        Ok(if self.resolve(repo, &base_ref)?.as_deref() == Some(old) {
            Step::Done(MergeOutcome::BaseCheckoutDirty { path: checkout })
        } else {
            Step::Moved
        })
    }

    fn attempt(&self, spec: &MergeSpec, number: u32) -> Result<Step, MergeError> {
        let repo = spec.repo.as_str();
        let old = self
            .resolve(repo, &format!("refs/heads/{}", spec.base))?
            .ok_or_else(|| MergeError::BaseMissing(spec.base.clone()))?;
        let theirs = self
            .resolve(repo, &format!("refs/heads/{}", spec.branch))?
            .ok_or_else(|| MergeError::BranchMissing(spec.branch.clone()))?;
        if self
            .git
            .succeeds(repo, &["merge-base", "--is-ancestor", &theirs, &old])?
        {
            return Ok(Step::Done(MergeOutcome::UpToDate));
        }
        let tree = match self.merge_tree(repo, &old, &theirs)? {
            Ok(tree) => tree,
            Err(files) => return Ok(Step::Done(MergeOutcome::Conflict { files })),
        };
        let new = self.commit_tree(spec, &tree, &old, &theirs)?;
        self.before_update(number);
        self.update_base(spec, &old, &new)
    }
}

impl Merger for GitMerger {
    fn merge(&self, spec: &MergeSpec) -> Result<MergeOutcome, MergeError> {
        let version = self.git.run(&spec.repo, &["--version"])?;
        check_version(&version)?;
        for number in 0..ATTEMPTS {
            if let Step::Done(outcome) = self.attempt(spec, number)? {
                return Ok(outcome);
            }
        }
        Err(MergeError::BaseMoved(spec.base.clone()))
    }
}

fn merged(commit: &str) -> Step {
    Step::Done(MergeOutcome::Merged {
        commit: commit.to_owned(),
    })
}

/// The path of the worktree that has `base_ref` checked out, from `git worktree list --porcelain`.
fn checkout_of(listing: &str, base_ref: &str) -> Option<String> {
    let mut path = None;
    for line in listing.lines() {
        if let Some(p) = line.strip_prefix("worktree ") {
            path = Some(p.to_owned());
        } else if line.strip_prefix("branch ") == Some(base_ref) {
            return path;
        }
    }
    None
}

/// Refuses a git older than [`MIN_GIT`]. Output that cannot be read is let through: the commands
/// themselves fail with git's own message if they are not supported.
fn check_version(text: &str) -> Result<(), MergeError> {
    match git_version(text) {
        Some(found) if found < MIN_GIT => Err(MergeError::GitTooOld {
            found: format!("{}.{}", found.0, found.1),
            needs: format!("{}.{}", MIN_GIT.0, MIN_GIT.1),
        }),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests;
