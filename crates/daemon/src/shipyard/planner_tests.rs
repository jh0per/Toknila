//! The planner's place to read the code, against real repositories.

use std::path::{Path, PathBuf};

use toknila_core::{BaseBehind, PlannerSpec};

use super::tests::{repo, yard};
use super::*;
use crate::git::testing::{commit_file, git};

/// `origin` (with `develop`) and a clone of it.
fn origin_and_clone(dir: &Path) -> (PathBuf, PathBuf) {
    let origin = PathBuf::from(repo(&dir.join("origin")));
    git(
        dir,
        &["clone", "--quiet", &origin.to_string_lossy(), "clone"],
    );
    (origin, dir.join("clone"))
}

fn spec_of(repo: &Path, remote: bool) -> PlannerSpec {
    PlannerSpec {
        repo: repo.to_string_lossy().into_owned(),
        base: "develop".to_owned(),
        remote,
        project: "demo".to_owned(),
    }
}

#[test]
fn in_github_mode_the_planner_reads_a_detached_checkout_of_the_fetched_base() {
    let tmp = tempfile::tempdir().expect("tmp");
    let (origin, clone) = origin_and_clone(tmp.path());
    commit_file(&origin, "merged-on-github.txt", "x\n", "merged");
    let yard = yard(tmp.path());
    let site = yard.planner_site(&spec_of(&clone, true));
    let dir = Path::new(&site.dir);
    assert!(dir.ends_with(Path::new("demo").join("_planner")), "{dir:?}");
    assert!(dir.starts_with(tmp.path().join("home dir").join("ships")));
    assert_eq!(site.behind, None);
    assert!(dir.join("merged-on-github.txt").is_file());
    assert_eq!(
        git(dir, &["rev-parse", "HEAD"]),
        git(&origin, &["rev-parse", "develop"])
    );
    assert_eq!(git(dir, &["rev-parse", "--abbrev-ref", "HEAD"]), "HEAD");
    // The owner's working copy was not touched.
    assert!(!clone.join("merged-on-github.txt").exists());
}

#[test]
fn every_start_moves_the_checkout_to_the_fresh_base_and_removal_takes_it_away() {
    let tmp = tempfile::tempdir().expect("tmp");
    let (origin, clone) = origin_and_clone(tmp.path());
    let yard = yard(tmp.path());
    let spec = spec_of(&clone, true);
    let first = yard.planner_site(&spec);
    commit_file(&origin, "second.txt", "x\n", "second");
    let second = yard.planner_site(&spec);
    assert_eq!(first.dir, second.dir);
    assert!(Path::new(&second.dir).join("second.txt").is_file());
    assert_eq!(
        git(Path::new(&second.dir), &["rev-parse", "HEAD"]),
        git(&origin, &["rev-parse", "develop"])
    );
    yard.remove_planner_site(&spec.repo, "demo")
        .expect("remove");
    assert!(!Path::new(&second.dir).exists());
    let listing = git(&clone, &["worktree", "list", "--porcelain"]);
    assert!(!listing.contains("_planner"), "{listing}");
    // Nothing to remove is fine.
    yard.remove_planner_site(&spec.repo, "demo").expect("again");
}

#[test]
fn without_a_remote_base_the_planner_stays_in_the_working_copy() {
    let tmp = tempfile::tempdir().expect("tmp");
    let alone = PathBuf::from(repo(tmp.path()));
    let site = yard(tmp.path()).planner_site(&spec_of(&alone, true));
    assert_eq!(site.dir, alone.to_string_lossy());
    assert_eq!(site.behind, None);
}

#[test]
fn in_local_mode_the_working_copy_is_used_and_a_stale_base_is_reported() {
    let tmp = tempfile::tempdir().expect("tmp");
    let (origin, clone) = origin_and_clone(tmp.path());
    let yard = yard(tmp.path());
    let spec = spec_of(&clone, false);
    let current = yard.planner_site(&spec);
    assert_eq!(current.dir, clone.to_string_lossy());
    assert_eq!(current.behind, None);
    commit_file(&origin, "a.txt", "x\n", "a");
    commit_file(&origin, "b.txt", "x\n", "b");
    let stale = yard.planner_site(&spec);
    assert_eq!(stale.dir, clone.to_string_lossy());
    assert_eq!(
        stale.behind,
        Some(BaseBehind {
            base: "develop".into(),
            upstream: "origin/develop".into(),
            behind: 2,
            can_fast_forward: true,
        })
    );
    assert!(!tmp.path().join("home dir").exists(), "no checkout made");
}
