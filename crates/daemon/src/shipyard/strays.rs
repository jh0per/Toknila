//! What the repository holds under Toknila's ships, and taking a leftover away.

use std::path::Path;

use toknila_app::ShipError;
use toknila_core::strays::{Stray, StrayKind};

use super::GitShipyard;
use crate::plan_files::canonical;

/// The worktrees in `git worktree list --porcelain` output as (path, branch) pairs; a detached
/// worktree has no branch.
fn parse_worktrees(text: &str) -> Vec<(String, Option<String>)> {
    let mut out: Vec<(String, Option<String>)> = Vec::new();
    for line in text.lines() {
        if let Some(path) = line.strip_prefix("worktree ") {
            out.push((path.to_owned(), None));
        } else if let Some(r) = line.strip_prefix("branch ")
            && let Some(last) = out.last_mut()
        {
            last.1 = Some(r.strip_prefix("refs/heads/").unwrap_or(r).to_owned());
        }
    }
    out
}

impl GitShipyard {
    pub(super) fn find_strays(&self, repo: &str, base: &str) -> Result<Vec<Stray>, ShipError> {
        let root = canonical(&self.root);
        let listing = self.git(repo, ["worktree", "list", "--porcelain"])?;
        let mut found: Vec<Stray> = parse_worktrees(&listing)
            .into_iter()
            .map(|(path, branch)| (canonical(Path::new(&path)), branch))
            // Under the ships folder, but never the planner's checkout (`<project>/_planner`).
            .filter(|(path, _)| {
                path.starts_with(&root) && !path.components().any(|c| c.as_os_str() == "_planner")
            })
            .map(|(path, branch)| Stray {
                kind: StrayKind::Worktree,
                name: super::path_text(&path),
                branch,
                ahead: None,
            })
            .collect();
        let refs = self.git(
            repo,
            [
                "for-each-ref",
                "--format=%(refname:short)",
                "refs/heads/ship/",
            ],
        )?;
        for name in refs.lines().map(str::trim).filter(|l| !l.is_empty()) {
            let ahead = self
                .commits_beyond(repo, base, &format!("refs/heads/{name}"))
                .ok();
            found.push(Stray {
                kind: StrayKind::Branch,
                name: name.to_owned(),
                branch: None,
                ahead,
            });
        }
        Ok(found)
    }

    pub(super) fn discard_stray(&self, repo: &str, stray: &Stray) -> Result<(), ShipError> {
        match stray.kind {
            StrayKind::Worktree => {
                if Path::new(&stray.name).exists() {
                    self.git(repo, ["worktree", "remove", "--force", &stray.name])?;
                }
                self.git(repo, ["worktree", "prune"])?;
            }
            StrayKind::Branch => {
                let branch_ref = format!("refs/heads/{}", stray.name);
                if self.succeeds(repo, ["rev-parse", "--verify", "--quiet", &branch_ref])? {
                    self.git(repo, ["branch", "-D", &stray.name])?;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use toknila_app::Shipyard;

    use super::super::tests::{repo, spec, yard};
    use super::*;
    use crate::git::testing::git;

    #[test]
    fn porcelain_gives_paths_and_branches() {
        let text =
            "worktree /r\nHEAD abc\nbranch refs/heads/main\n\nworktree /s/x\nHEAD def\ndetached\n";
        assert_eq!(
            parse_worktrees(text),
            [
                ("/r".to_owned(), Some("main".to_owned())),
                ("/s/x".to_owned(), None)
            ]
        );
    }

    #[test]
    fn the_yard_finds_its_ships_and_ship_branches_but_not_the_main_checkout() {
        let tmp = tempfile::tempdir().expect("tmp");
        let repo = repo(tmp.path());
        let spec = spec(&repo);
        let yard = yard(tmp.path());
        let path = yard.prepare(&spec).expect("prepare");
        let found = yard.found(&repo, "develop").expect("found");
        let trees: Vec<_> = found
            .iter()
            .filter(|s| s.kind == StrayKind::Worktree)
            .collect();
        assert_eq!(trees.len(), 1, "{found:?}");
        assert_eq!(trees[0].branch.as_deref(), Some(spec.branch.as_str()));
        assert_eq!(
            canonical(Path::new(&trees[0].name)),
            canonical(Path::new(&path))
        );
        let branches: Vec<_> = found
            .iter()
            .filter(|s| s.kind == StrayKind::Branch)
            .collect();
        assert_eq!(branches.len(), 1, "develop is not a ship branch: {found:?}");
        assert_eq!(branches[0].name, spec.branch);
        assert_eq!(branches[0].ahead, Some(0));
    }

    #[test]
    fn discarding_removes_the_worktree_then_the_branch_even_with_commits() {
        let tmp = tempfile::tempdir().expect("tmp");
        let repo = repo(tmp.path());
        let spec = spec(&repo);
        let yard = yard(tmp.path());
        let path = yard.prepare(&spec).expect("prepare");
        std::fs::write(Path::new(&path).join("w.txt"), "x").expect("write");
        git(Path::new(&path), &["add", "w.txt"]);
        git(
            Path::new(&path),
            &[
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "commit",
                "-m",
                "w",
            ],
        );
        let found = yard.found(&repo, "develop").expect("found");
        let branch = found
            .iter()
            .find(|s| s.kind == StrayKind::Branch)
            .expect("branch");
        assert_eq!(branch.ahead, Some(1));
        for s in found.iter().filter(|s| s.kind == StrayKind::Worktree) {
            yard.discard(&repo, s).expect("worktree");
        }
        yard.discard(&repo, branch).expect("branch");
        yard.discard(&repo, branch).expect("again is fine");
        assert!(yard.found(&repo, "develop").expect("found").is_empty());
        assert!(!Path::new(&path).exists());
    }
}
