//! Where a project's planner reads the code. In pull-request mode merges happen on GitHub, so the
//! working copy goes stale; the planner gets a detached checkout of the fetched `origin/<base>`
//! at `<ships root>/<project>/_planner` (a name no chain can have: chain folders are made of
//! lower-case letters, digits and dashes), moved to the fresh base on every start. In local mode
//! the working copy is the truth, and the planner is told when its base is behind its upstream.

use std::path::PathBuf;

use toknila_app::ShipError;
use toknila_core::{PlannerSite, PlannerSpec};

use super::{GitShipyard, is_worktree, path_text};

/// The folder name of the planner's checkout under the project's folder.
const FOLDER: &str = "_planner";

impl GitShipyard {
    fn planner_path(&self, project: &str) -> PathBuf {
        self.root.join(project).join(FOLDER)
    }

    /// The planner's place for `spec`; every failure falls back to the repository as it is.
    pub(super) fn site_for(&self, spec: &PlannerSpec) -> PlannerSite {
        let repo = spec.repo.as_str();
        let fetch: &[&str] = if spec.remote {
            &["--quiet", "origin"]
        } else {
            &["--quiet"]
        };
        let _ = self.git.fetch(repo, fetch);
        if spec.remote {
            let dir = self
                .fresh_checkout(spec)
                .unwrap_or_else(|_| repo.to_owned());
            return PlannerSite { dir, behind: None };
        }
        PlannerSite {
            dir: repo.to_owned(),
            behind: self.base_behind(repo, &spec.base).ok().flatten(),
        }
    }

    /// The planner's checkout, made or moved to the fetched base.
    fn fresh_checkout(&self, spec: &PlannerSpec) -> Result<String, ShipError> {
        let repo = spec.repo.as_str();
        let start = format!("origin/{}", spec.base);
        let commit = format!("{start}^{{commit}}");
        let found = !start.starts_with('-')
            && self.succeeds(repo, ["rev-parse", "--verify", "--quiet", &commit])?;
        if !found {
            return Err(ShipError::BaseBranchMissing(start));
        }
        let path = self.planner_path(&spec.project);
        let at = path_text(&path);
        if is_worktree(&path) {
            self.git(&at, ["checkout", "--quiet", "--force", "--detach", &commit])?;
            return Ok(at);
        }
        if path.exists() && std::fs::remove_dir(&path).is_err() {
            return Err(ShipError::PathTaken(at));
        }
        self.git(repo, ["worktree", "prune"])?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| ShipError::Io(format!("cannot create {}: {e}", parent.display())))?;
        }
        self.git(
            repo,
            ["worktree", "add", "--quiet", "--detach", &at, &commit],
        )?;
        Ok(at)
    }

    /// Removes the planner's checkout of `project`.
    pub(super) fn remove_site(&self, repo: &str, project: &str) -> Result<(), ShipError> {
        let path = self.planner_path(project);
        if path.exists() {
            self.git(repo, ["worktree", "remove", "--force", &path_text(&path)])?;
        }
        self.git(repo, ["worktree", "prune"])?;
        let _ = std::fs::remove_dir(self.root.join(project));
        Ok(())
    }
}
