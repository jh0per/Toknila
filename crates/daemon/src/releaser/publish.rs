//! The steps of a release run that change the repository for everyone: the tag on the base
//! branch, and the push of both to the remote.

use toknila_core::release_run::{push_failure_line, pushed_text};

use super::{Ctx, PUSH_LIMIT};
use crate::git::GitError;
use crate::merger::GitMerger;

impl Ctx<'_> {
    pub(super) fn tag(&self) -> Result<String, String> {
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
                // No card merges while a release holds the merge line: what moved the branch was
                // done outside Toknila.
                Ok(false) => {
                    return Err(undo(format!(
                        "{} moved while the release ran, by a change made outside Toknila (a push by hand?); nothing was tagged. Release again",
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

    pub(super) fn push(&self) -> Result<String, String> {
        let plan = self.plan;
        let Some(remote) = &plan.remote else {
            return Err("the repository has no remote to push to".to_owned());
        };
        let args = [
            "push",
            "--porcelain",
            remote.name.as_str(),
            &format!("refs/heads/{}", plan.base),
            &format!("refs/tags/{}", plan.tag),
        ];
        match self.git.run_within(&plan.repo, &args, PUSH_LIMIT) {
            Ok(_) => {
                // The tag's commit is the one CI must pass on.
                let short = self
                    .git
                    .run(
                        &plan.repo,
                        &["rev-parse", "--short", &format!("{}^{{commit}}", plan.tag)],
                    )
                    .map(|s| s.trim().to_owned())
                    .unwrap_or_default();
                Ok(pushed_text(
                    &plan.base,
                    &plan.tag,
                    &remote.name,
                    &remote.url,
                    &short,
                ))
            }
            // The detail holds git's stderr and the porcelain report (the refused ref); the last
            // line is the one a person reads.
            Err(GitError::Failed { command, detail }) => {
                let line = push_failure_line(&detail);
                Err(format!("$ git {command}\n{detail}\n{line}"))
            }
            Err(e) => Err(e.to_string()),
        }
    }
}
