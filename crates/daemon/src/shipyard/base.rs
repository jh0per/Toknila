//! Local merge mode: is the base branch behind its upstream? The facts come from `git`; what to do
//! about them is core's call ([`BaseBehind::course`]).

use toknila_app::ShipError;
use toknila_core::{BaseBehind, BaseCourse, ShipSpec};

use super::GitShipyard;
use crate::merger::checkout_of;

impl GitShipyard {
    /// Before a ship is made from the local base branch: fetch (a failure, offline say, is fine),
    /// and if the base is behind its upstream do what the spec's policy allows, or refuse with the
    /// facts. A base without an upstream, or not behind, passes.
    pub(super) fn check_base(&self, spec: &ShipSpec) -> Result<(), ShipError> {
        let repo = spec.repo.as_str();
        let _ = self.git.fetch(repo, &["--quiet"]);
        let Some(found) = self.base_behind(repo, &spec.base)? else {
            return Ok(());
        };
        match found.course(spec.stale) {
            BaseCourse::Start => Ok(()),
            BaseCourse::Refuse => Err(ShipError::BaseBehind(found)),
            BaseCourse::FastForwardThenStart => self.fast_forward(spec, &found),
        }
    }

    /// The facts about the base branch, or `None` when it has no upstream or is not behind.
    pub(super) fn base_behind(
        &self,
        repo: &str,
        base: &str,
    ) -> Result<Option<BaseBehind>, ShipError> {
        let base_ref = format!("refs/heads/{base}");
        let upstream_spec = format!("{base}@{{upstream}}");
        // No upstream configured: nothing to be behind.
        let Ok(upstream) = self.git(repo, ["rev-parse", "--abbrev-ref", &upstream_spec]) else {
            return Ok(None);
        };
        let range = format!("{base}..{upstream}");
        let behind: u32 = self
            .git(repo, ["rev-list", "--count", &range])?
            .parse()
            .unwrap_or(0);
        if behind == 0 {
            return Ok(None);
        }
        let own_commits = !self.succeeds(repo, ["merge-base", "--is-ancestor", base, &upstream])?;
        let listing = self.git(repo, ["worktree", "list", "--porcelain"])?;
        let dirty = match checkout_of(&listing, &base_ref) {
            Some(checkout) => !self
                .git(&checkout, ["status", "--porcelain", "--untracked-files=no"])?
                .is_empty(),
            None => false,
        };
        Ok(Some(BaseBehind {
            base: base.to_owned(),
            upstream,
            behind,
            can_fast_forward: !own_commits && !dirty,
        }))
    }

    /// Moves the base to its upstream: in the checkout that has it when there is one, else by
    /// moving the ref.
    fn fast_forward(&self, spec: &ShipSpec, found: &BaseBehind) -> Result<(), ShipError> {
        let repo = spec.repo.as_str();
        let base_ref = format!("refs/heads/{}", spec.base);
        let listing = self.git(repo, ["worktree", "list", "--porcelain"])?;
        match checkout_of(&listing, &base_ref) {
            Some(checkout) => {
                self.git(&checkout, ["merge", "--ff-only", found.upstream.as_str()])?;
            }
            None => {
                let new = self.git(
                    repo,
                    ["rev-parse", &format!("{}^{{commit}}", found.upstream)],
                )?;
                let old = self.git(repo, ["rev-parse", &base_ref])?;
                self.git(repo, ["update-ref", &base_ref, &new, &old])?;
            }
        }
        Ok(())
    }
}
