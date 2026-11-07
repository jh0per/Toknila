//! Local mode: a base branch that is behind its upstream is not started from silently.

use std::path::{Path, PathBuf};

use toknila_core::{BaseBehind, StalePolicy};

use super::tests::{repo, spec, yard};
use super::*;
use crate::git::testing::{commit_file, git};

/// `origin` (a repository with `develop`) and a clone of it that is `behind` commits behind.
fn clone_behind(dir: &Path, behind: usize) -> (PathBuf, String) {
    let origin = PathBuf::from(repo(&dir.join("origin")));
    let local = dir.join("clone");
    git(
        dir,
        &["clone", "--quiet", &origin.to_string_lossy(), "clone"],
    );
    for n in 0..behind {
        commit_file(&origin, &format!("new-{n}.txt"), "x\n", &format!("new {n}"));
    }
    (origin, local.to_string_lossy().into_owned())
}

fn behind_of(err: ShipError) -> BaseBehind {
    let ShipError::BaseBehind(found) = err else {
        panic!("not a base-behind refusal: {err:?}");
    };
    found
}

#[test]
fn a_base_behind_its_upstream_refuses_the_start_with_the_facts_and_makes_nothing() {
    let tmp = tempfile::tempdir().expect("tmp");
    let (_origin, local) = clone_behind(tmp.path(), 3);
    let yard = yard(tmp.path());
    let found = behind_of(yard.prepare(&spec(&local)).expect_err("refused"));
    assert_eq!(
        found,
        BaseBehind {
            base: "develop".into(),
            upstream: "origin/develop".into(),
            behind: 3,
            can_fast_forward: true,
        }
    );
    assert_eq!(
        ShipError::BaseBehind(found).to_string(),
        "base branch develop is 3 commits behind origin/develop"
    );
    assert!(!yard.root.exists(), "no ship folder was made");
}

#[test]
fn fast_forward_and_start_moves_the_base_and_the_checkout_then_makes_the_ship() {
    let tmp = tempfile::tempdir().expect("tmp");
    let (_origin, local) = clone_behind(tmp.path(), 2);
    let mut spec = spec(&local);
    spec.stale = StalePolicy::FastForward;
    let path = yard(tmp.path()).prepare(&spec).expect("prepare");
    assert!(Path::new(&path).join("new-1.txt").is_file());
    assert!(Path::new(&local).join("new-1.txt").is_file());
    assert_eq!(
        git(Path::new(&local), &["rev-parse", "develop"]),
        git(Path::new(&local), &["rev-parse", "origin/develop"])
    );
}

#[test]
fn a_base_that_is_not_checked_out_is_fast_forwarded_by_moving_the_ref() {
    let tmp = tempfile::tempdir().expect("tmp");
    let (_origin, local) = clone_behind(tmp.path(), 1);
    git(Path::new(&local), &["checkout", "--quiet", "-b", "other"]);
    let mut spec = spec(&local);
    spec.stale = StalePolicy::FastForward;
    let path = yard(tmp.path()).prepare(&spec).expect("prepare");
    assert!(Path::new(&path).join("new-0.txt").is_file());
}

#[test]
fn a_dirty_checkout_or_own_commits_cannot_be_fast_forwarded() {
    let tmp = tempfile::tempdir().expect("tmp");
    let (_origin, local) = clone_behind(tmp.path(), 1);
    std::fs::write(Path::new(&local).join("README.md"), "edited\n").expect("write");
    let mut spec = spec(&local);
    let found = behind_of(yard(tmp.path()).prepare(&spec).expect_err("dirty"));
    assert!(!found.can_fast_forward);
    spec.stale = StalePolicy::FastForward;
    let again = behind_of(yard(tmp.path()).prepare(&spec).expect_err("still refused"));
    assert!(!again.can_fast_forward);
    assert_eq!(
        std::fs::read_to_string(Path::new(&local).join("README.md")).expect("read"),
        "edited\n",
        "the person's change is untouched"
    );

    git(
        Path::new(&local),
        &["checkout", "--quiet", "--", "README.md"],
    );
    commit_file(Path::new(&local), "mine.txt", "x\n", "mine");
    let diverged = behind_of(yard(tmp.path()).prepare(&spec).expect_err("diverged"));
    assert!(!diverged.can_fast_forward);
}

#[test]
fn start_anyway_makes_the_ship_from_the_base_as_it_is() {
    let tmp = tempfile::tempdir().expect("tmp");
    let (_origin, local) = clone_behind(tmp.path(), 2);
    let mut spec = spec(&local);
    spec.stale = StalePolicy::Allow;
    let path = yard(tmp.path()).prepare(&spec).expect("prepare");
    assert!(Path::new(&path).join("README.md").is_file());
    assert!(!Path::new(&path).join("new-0.txt").exists());
}

#[test]
fn offline_or_without_an_upstream_the_start_goes_on() {
    let tmp = tempfile::tempdir().expect("tmp");
    let (origin, local) = clone_behind(tmp.path(), 0);
    std::fs::remove_dir_all(&origin).expect("origin gone");
    let first = yard(tmp.path()).prepare(&spec(&local)).expect("offline");
    assert!(Path::new(&first).join("README.md").is_file());

    let lone = repo(&tmp.path().join("lone"));
    let mut other = spec(&lone);
    other.branch = "ship/auth/other".into();
    other.slug = "other".into();
    yard(tmp.path()).prepare(&other).expect("no upstream");
}
