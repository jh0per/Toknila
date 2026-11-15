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
        if ahead > 0 && !self.taken_by(repo, start, &branch_ref)? {
            return Ok(BranchFate::Kept { ahead });
        }
        self.git(repo, ["branch", "-D", &ship.branch])?;
        Ok(BranchFate::Deleted)
    }

    /// Whether `start` already holds everything the branch changed: a squash or rebase merge
    /// leaves the branch's commits out of `start`'s history but its files in it. False when the
    /// branch changed nothing, or any file it changed differs from `start` now.
    fn taken_by(&self, repo: &str, start: &str, branch_ref: &str) -> Result<bool, ShipError> {
        let Ok(base) = self.git(repo, ["merge-base", start, branch_ref]) else {
            return Ok(false);
        };
        let names = self.git(repo, ["diff", "--name-only", "-z", &base, branch_ref, "--"])?;
        let files: Vec<&str> = names.split('\0').filter(|f| !f.is_empty()).collect();
        if files.is_empty() {
            return Ok(false);
        }
        let mut args = vec!["diff", "--quiet", start, branch_ref, "--"];
        args.extend(files);
        self.succeeds(repo, args)
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

    fn commit(dir: &Path, file: &str, text: &str) {
        std::fs::write(dir.join(file), text).expect("write");
        git(dir, &["add", file]);
        git(
            dir,
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
    }

    #[test]
    fn a_branch_whose_work_the_base_took_as_a_squash_goes_and_a_changed_one_stays() {
        for (base_text, fate) in [
            ("x", BranchFate::Deleted),
            ("changed on the base later", BranchFate::Kept { ahead: 1 }),
        ] {
            let tmp = tempfile::tempdir().expect("tmp");
            let repo = repo(tmp.path());
            let spec = spec(&repo);
            let yard = yard(tmp.path());
            let path = yard.prepare(&spec).expect("prepare");
            commit(Path::new(&path), "work.txt", "x");
            // The pull request was squashed: the base has the work as a commit of its own.
            commit(Path::new(&repo), "work.txt", base_text);
            let ship = ship_for(&spec, path.clone());
            assert_eq!(yard.retire(&repo, &ship, "develop").expect("retire"), fate);
            assert!(!Path::new(&path).exists());
        }
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
