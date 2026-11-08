//! Retiring a cancelled ship: the worktree goes, the branch goes only when it holds no commits of
//! its own.

use toknila_app::{BranchFate, ShipError};
use toknila_core::Ship;

use super::GitShipyard;

impl GitShipyard {
    pub(super) fn retire_ship(
        &self,
        repo: &str,
        ship: &Ship,
        start: &str,
    ) -> Result<BranchFate, ShipError> {
        use toknila_app::Shipyard;
        self.remove(repo, ship)?;
        let branch_ref = format!("refs/heads/{}", ship.branch);
        if !self.succeeds(repo, ["rev-parse", "--verify", "--quiet", &branch_ref])? {
            return Ok(BranchFate::Deleted);
        }
        let ahead = self.commits_beyond(repo, start, &branch_ref)?;
        if ahead > 0 {
            return Ok(BranchFate::Kept { ahead });
        }
        self.git(repo, ["branch", "-D", &ship.branch])?;
        Ok(BranchFate::Deleted)
    }

    /// How many commits `branch_ref` has that `start` has not.
    pub(super) fn commits_beyond(
        &self,
        repo: &str,
        start: &str,
        branch_ref: &str,
    ) -> Result<u32, ShipError> {
        let range = format!("{start}..{branch_ref}");
        let count = self.git(repo, ["rev-list", "--count", range.as_str()])?;
        count.parse().map_err(|_| {
            ShipError::Git(toknila_app::GitError::Failed {
                command: "rev-list --count".to_owned(),
                detail: format!("unexpected answer {count:?}"),
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use toknila_app::Shipyard;

    use super::super::tests::{repo, spec, yard};
    use super::*;
    use crate::git::testing::git;

    fn ship_for(spec: &toknila_core::ShipSpec, path: String) -> Ship {
        Ship {
            ticket_id: toknila_core::TicketId::new(),
            key: spec.key.clone(),
            branch: spec.branch.clone(),
            path,
        }
    }

    #[test]
    fn an_empty_branch_goes_with_its_worktree() {
        let tmp = tempfile::tempdir().expect("tmp");
        let repo = repo(tmp.path());
        let spec = spec(&repo);
        let yard = yard(tmp.path());
        let path = yard.prepare(&spec).expect("prepare");
        let ship = ship_for(&spec, path.clone());
        let fate = yard.retire(&repo, &ship, "develop").expect("retire");
        assert_eq!(fate, BranchFate::Deleted);
        assert!(!Path::new(&path).exists());
        assert_eq!(
            git(Path::new(&repo), &["branch", "--list", &spec.branch]),
            ""
        );
    }

    #[test]
    fn a_branch_with_a_commit_stays_and_says_how_many() {
        let tmp = tempfile::tempdir().expect("tmp");
        let repo = repo(tmp.path());
        let spec = spec(&repo);
        let yard = yard(tmp.path());
        let path = yard.prepare(&spec).expect("prepare");
        std::fs::write(Path::new(&path).join("work.txt"), "x").expect("write");
        git(Path::new(&path), &["add", "work.txt"]);
        git(
            Path::new(&path),
            &[
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "commit",
                "-m",
                "work",
            ],
        );
        let ship = ship_for(&spec, path.clone());
        let fate = yard.retire(&repo, &ship, "develop").expect("retire");
        assert_eq!(fate, BranchFate::Kept { ahead: 1 });
        assert!(!Path::new(&path).exists());
        assert!(
            git(Path::new(&repo), &["branch", "--list", &spec.branch]).contains(&spec.branch),
            "the branch with work survives"
        );
    }

    #[test]
    fn a_ship_that_is_already_gone_retires_without_error() {
        let tmp = tempfile::tempdir().expect("tmp");
        let repo = repo(tmp.path());
        let spec = spec(&repo);
        let ship = ship_for(
            &spec,
            tmp.path().join("nowhere").to_string_lossy().into_owned(),
        );
        let fate = yard(tmp.path())
            .retire(&repo, &ship, "develop")
            .expect("retire");
        assert_eq!(fate, BranchFate::Deleted);
    }
}
