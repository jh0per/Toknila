//! Bringing a ship up to its base: `git merge` of the base into the ship's worktree, after a
//! look that nothing conflicts (`merge-tree`, which touches nothing) and that no schema migration
//! number is taken by both branches. A ship that would conflict is left exactly as it was.

use toknila_app::{MergeError, SyncOutcome, SyncSpec};
use toknila_core::migrations::{self, SCHEMA_FILE};

use super::GitMerger;

impl GitMerger {
    /// The migrations `reference` added to the schema file since `fork`.
    fn migrations_added(
        &self,
        repo: &str,
        fork: &str,
        reference: &str,
    ) -> Result<Vec<migrations::Migration>, MergeError> {
        let diff = self
            .git
            .run(repo, &["diff", "-U0", fork, reference, "--", SCHEMA_FILE])?;
        Ok(migrations::added(&diff))
    }

    /// Merges `spec.base` into the ship at `spec.path` when the ship does not contain it yet.
    pub(super) fn sync_ship(&self, spec: &SyncSpec) -> Result<SyncOutcome, MergeError> {
        let dir = spec.path.as_str();
        let base = spec.base.as_str();
        if self
            .git
            .succeeds(dir, &["merge-base", "--is-ancestor", base, "HEAD"])?
        {
            return Ok(SyncOutcome::UpToDate);
        }
        let changes = self
            .git
            .run(dir, &["status", "--porcelain", "--untracked-files=no"])?;
        if !changes.is_empty() {
            return Err(MergeError::Io(
                "the ship has changes that are not committed".to_owned(),
            ));
        }
        let fork = self.git.run(dir, &["merge-base", "HEAD", base])?;
        let clashes = migrations::clashes(
            &self.migrations_added(dir, &fork, "HEAD")?,
            &self.migrations_added(dir, &fork, base)?,
        );
        let files = match self.merge_tree(dir, "HEAD", base)? {
            Ok(_) => Vec::new(),
            Err(files) => files,
        };
        if !clashes.is_empty() || !files.is_empty() {
            return Ok(SyncOutcome::Conflict {
                files,
                migrations: clashes,
            });
        }
        let mut args = Vec::new();
        if !self.git.succeeds(dir, &["var", "GIT_COMMITTER_IDENT"])? {
            args.extend([
                "-c",
                "user.name=Toknila",
                "-c",
                "user.email=toknila@localhost",
            ]);
        }
        args.extend(["merge", "--no-ff", "--no-edit", "-m", &spec.message, base]);
        self.git.run(dir, &args)?;
        let commit = self.git.run(dir, &["rev-parse", "HEAD"])?;
        Ok(SyncOutcome::Merged { commit })
    }
}
