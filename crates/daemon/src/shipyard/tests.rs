//! The git shipyard against real temporary repositories (a `develop` branch with one commit).

use std::path::Path;

use toknila_core::{StalePolicy, TicketId};

use super::*;
use crate::git::testing::{git, init_repo};

/// A repository whose only branch is `develop`, with one commit.
pub(super) fn repo(dir: &Path) -> String {
    let repo = dir.join("the repo");
    init_repo(&repo, "develop");
    repo.to_string_lossy().into_owned()
}

pub(super) fn spec(repo: &str) -> ShipSpec {
    ShipSpec {
        key: "DEMO-1".parse().expect("key"),
        repo: repo.to_owned(),
        base: "develop".to_owned(),
        remote: false,
        stale: StalePolicy::default(),
        branch: "ship/auth/demo-1-login".to_owned(),
        project: "demo".to_owned(),
        chain: "auth".to_owned(),
        slug: "demo-1-login".to_owned(),
    }
}

fn ship_of(spec: &ShipSpec, path: String) -> Ship {
    Ship {
        ticket_id: TicketId::new(),
        key: "DEMO-1".parse().expect("key"),
        branch: spec.branch.clone(),
        path,
    }
}

/// A shipyard under a folder with a space in its name, as a user's home may have.
pub(super) fn yard(dir: &Path) -> GitShipyard {
    GitShipyard::new(dir.join("home dir").join("ships"))
}

#[test]
fn prepare_makes_a_worktree_on_a_new_branch_from_the_base() {
    let tmp = tempfile::tempdir().expect("tmp");
    let repo = repo(tmp.path());
    let spec = spec(&repo);
    let path = yard(tmp.path()).prepare(&spec).expect("prepare");
    let path = Path::new(&path);
    assert!(path.join("README.md").is_file(), "{path:?}");
    assert!(path.ends_with(Path::new("demo").join("auth").join("demo-1-login")));
    assert_eq!(
        git(path, &["rev-parse", "--abbrev-ref", "HEAD"]),
        "ship/auth/demo-1-login"
    );
    assert_eq!(
        git(path, &["rev-parse", "HEAD"]),
        git(Path::new(&repo), &["rev-parse", "develop"])
    );
}

#[test]
fn prepare_twice_reuses_the_ship() {
    let tmp = tempfile::tempdir().expect("tmp");
    let repo = repo(tmp.path());
    let spec = spec(&repo);
    let yard = yard(tmp.path());
    let first = yard.prepare(&spec).expect("first");
    std::fs::write(Path::new(&first).join("wip.txt"), "x").expect("write");
    let again = yard.prepare(&spec).expect("again");
    assert_eq!(first, again);
    assert!(Path::new(&again).join("wip.txt").is_file());
}

#[test]
fn remove_deletes_the_worktree_and_keeps_the_branch_and_its_commits() {
    let tmp = tempfile::tempdir().expect("tmp");
    let repo = repo(tmp.path());
    let spec = spec(&repo);
    let yard = yard(tmp.path());
    let path = yard.prepare(&spec).expect("prepare");
    std::fs::write(Path::new(&path).join("work.txt"), "done\n").expect("write");
    git(Path::new(&path), &["add", "work.txt"]);
    git(Path::new(&path), &["commit", "--quiet", "-m", "work"]);
    std::fs::write(Path::new(&path).join("untracked.txt"), "x").expect("write");

    yard.remove(&repo, &ship_of(&spec, path.clone()))
        .expect("remove even with untracked files");
    assert!(!Path::new(&path).exists());
    assert!(
        !yard.root.join("demo").exists(),
        "empty project and chain folders are tidied"
    );
    assert_eq!(
        git(
            Path::new(&repo),
            &["branch", "--list", "ship/auth/demo-1-login"]
        )
        .trim_start_matches(['*', ' ']),
        "ship/auth/demo-1-login"
    );
    yard.remove(&repo, &ship_of(&spec, path.clone()))
        .expect("removing twice is fine");

    let back = yard.prepare(&spec).expect("prepare again");
    assert!(
        Path::new(&back).join("work.txt").is_file(),
        "the branch kept the commit"
    );
}

#[test]
fn a_worktree_deleted_by_hand_is_made_again() {
    let tmp = tempfile::tempdir().expect("tmp");
    let repo = repo(tmp.path());
    let spec = spec(&repo);
    let yard = yard(tmp.path());
    let path = yard.prepare(&spec).expect("prepare");
    std::fs::remove_dir_all(&path).expect("rm");
    let again = yard.prepare(&spec).expect("prepare after rm");
    assert!(Path::new(&again).join("README.md").is_file());
}

#[test]
fn a_missing_git_is_a_typed_error() {
    let tmp = tempfile::tempdir().expect("tmp");
    let repo = repo(tmp.path());
    let yard = GitShipyard::with_program(tmp.path().join("ships"), "toknila-no-such-git");
    assert_eq!(
        yard.prepare(&spec(&repo)),
        Err(ShipError::Git(toknila_app::GitError::Missing))
    );
}

#[test]
fn a_folder_that_is_not_a_repository_is_a_typed_error() {
    let tmp = tempfile::tempdir().expect("tmp");
    let plain = tmp.path().join("plain");
    std::fs::create_dir_all(&plain).expect("mkdir");
    let plain = plain.to_string_lossy().into_owned();
    assert_eq!(
        yard(tmp.path()).prepare(&spec(&plain)),
        Err(ShipError::NotARepository(plain))
    );
}

#[test]
fn a_missing_base_branch_is_a_typed_error() {
    let tmp = tempfile::tempdir().expect("tmp");
    let repo = repo(tmp.path());
    for base in ["nope", "--orphan"] {
        let spec = ShipSpec {
            base: base.to_owned(),
            ..spec(&repo)
        };
        assert_eq!(
            yard(tmp.path()).prepare(&spec),
            Err(ShipError::BaseBranchMissing(base.to_owned()))
        );
    }
}

#[test]
fn a_path_held_by_something_else_is_not_overwritten() {
    let tmp = tempfile::tempdir().expect("tmp");
    let repo = repo(tmp.path());
    let spec = spec(&repo);
    let yard = yard(tmp.path());
    let taken = yard.worktree_path(&spec);
    std::fs::create_dir_all(&taken).expect("mkdir");
    std::fs::write(taken.join("mine.txt"), "x").expect("write");
    assert_eq!(
        yard.prepare(&spec),
        Err(ShipError::PathTaken(taken.to_string_lossy().into_owned()))
    );
}

#[test]
fn a_ship_in_pull_request_mode_starts_from_the_fetched_origin_base() {
    let tmp = tempfile::tempdir().expect("tmp");
    let repo = repo(tmp.path());
    let origin = tmp.path().join("origin.git");
    git(tmp.path(), &["init", "--quiet", "--bare", "origin.git"]);
    git(
        Path::new(&repo),
        &["remote", "add", "origin", &origin.to_string_lossy()],
    );
    git(Path::new(&repo), &["push", "--quiet", "origin", "develop"]);
    git(&origin, &["symbolic-ref", "HEAD", "refs/heads/develop"]);
    // Someone else's merge moved develop on the remote; the local branch is behind.
    let other = tmp.path().join("other");
    git(
        tmp.path(),
        &["clone", "--quiet", &origin.to_string_lossy(), "other"],
    );
    crate::git::testing::commit_file(&other, "new.txt", "n\n", "merged elsewhere");
    git(&other, &["push", "--quiet", "origin", "develop"]);
    let tip = git(&other, &["rev-parse", "HEAD"]);

    let spec = ShipSpec {
        remote: true,
        ..spec(&repo)
    };
    let path = yard(tmp.path()).prepare(&spec).expect("prepare");
    assert_eq!(git(Path::new(&path), &["rev-parse", "HEAD"]), tip);
    assert!(Path::new(&path).join("new.txt").is_file());
}
