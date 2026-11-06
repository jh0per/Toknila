//! The git merger against real temporary repositories: `develop` is the base, `ship/x` the
//! branch of a card that lives in its own worktree, as in real use.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use toknila_core::MergeSpec;

use super::*;
use crate::git::testing::{commit_file, git};

struct Fixture {
    _tmp: tempfile::TempDir,
    /// The person's checkout, on `develop` unless a test says otherwise.
    repo: PathBuf,
    /// The card's worktree on `ship/x`, with one commit adding `feature.txt`.
    ship: PathBuf,
}

fn setup() -> Fixture {
    let tmp = tempfile::tempdir().expect("tmp");
    let repo = tmp.path().join("the repo");
    std::fs::create_dir_all(&repo).expect("mkdir");
    git(&repo, &["init", "--quiet"]);
    git(&repo, &["symbolic-ref", "HEAD", "refs/heads/develop"]);
    commit_file(&repo, "README.md", "hi\n", "init");
    let ship = tmp.path().join("ship");
    git(
        &repo,
        &["worktree", "add", "-b", "ship/x", &ship.to_string_lossy()],
    );
    commit_file(&ship, "feature.txt", "feature\n", "add feature");
    Fixture {
        _tmp: tmp,
        repo,
        ship,
    }
}

fn spec(f: &Fixture) -> MergeSpec {
    MergeSpec {
        key: "DEMO-1".parse().expect("key"),
        repo: f.repo.to_string_lossy().into_owned(),
        base: "develop".to_owned(),
        branch: "ship/x".to_owned(),
        message: "Merge ship/x: DEMO-1 Add feature".to_owned(),
        pr: None,
        look: toknila_core::ShipLook {
            path: f.ship.to_string_lossy().into_owned(),
            start: "develop".to_owned(),
        },
    }
}

fn tip(dir: &Path, branch: &str) -> String {
    git(dir, &["rev-parse", &format!("refs/heads/{branch}")])
}

fn same_path(a: &str, b: &Path) -> bool {
    std::fs::canonicalize(a).ok() == std::fs::canonicalize(b).ok()
}

fn merged_commit(outcome: Result<MergeOutcome, MergeError>) -> String {
    match outcome {
        Ok(MergeOutcome::Merged { commit }) => commit,
        other => panic!("expected a merge, got {other:?}"),
    }
}

/// Asserts the base tip is a merge commit of `old` and the ship, with the card's message.
fn assert_merge_commit(f: &Fixture, commit: &str, old: &str) {
    assert_eq!(tip(&f.repo, "develop"), commit);
    let parents = git(&f.repo, &["rev-list", "--parents", "-n", "1", commit]);
    let parents: Vec<&str> = parents.split(' ').collect();
    assert_eq!(parents.len(), 3, "a merge commit has two parents");
    assert_eq!(parents[1], old, "first parent is the old base");
    assert_eq!(parents[2], tip(&f.ship, "ship/x"), "second is the ship");
    assert_eq!(
        git(&f.repo, &["log", "-1", "--format=%s", commit]),
        "Merge ship/x: DEMO-1 Add feature"
    );
}

#[test]
fn a_clean_checked_out_base_is_fast_forwarded_to_a_merge_commit() {
    let f = setup();
    let old = tip(&f.repo, "develop");
    let commit = merged_commit(GitMerger::new().merge(&spec(&f)));
    assert_merge_commit(&f, &commit, &old);
    assert!(
        f.repo.join("feature.txt").is_file(),
        "the person's checkout got the files"
    );
    assert_eq!(git(&f.repo, &["status", "--porcelain"]), "");
    assert_eq!(git(&f.repo, &["rev-parse", "HEAD"]), commit);
}

#[test]
fn a_base_checked_out_with_local_changes_blocks_and_changes_nothing() {
    let f = setup();
    let old = tip(&f.repo, "develop");
    std::fs::write(f.repo.join("README.md"), "my work in progress\n").expect("write");
    let outcome = GitMerger::new().merge(&spec(&f)).expect("merge");
    let MergeOutcome::BaseCheckoutDirty { path } = outcome else {
        panic!("expected a dirty checkout, got {outcome:?}");
    };
    assert!(same_path(&path, &f.repo), "{path}");
    assert_eq!(tip(&f.repo, "develop"), old, "the branch did not move");
    assert_eq!(
        std::fs::read_to_string(f.repo.join("README.md")).expect("read"),
        "my work in progress\n"
    );
    assert!(!f.repo.join("feature.txt").exists());
}

#[test]
fn a_base_checked_out_in_another_worktree_is_fast_forwarded_there() {
    let f = setup();
    git(&f.repo, &["checkout", "--quiet", "-b", "main"]);
    let other = f.repo.parent().expect("parent").join("develop-checkout");
    git(
        &f.repo,
        &["worktree", "add", &other.to_string_lossy(), "develop"],
    );
    let old = tip(&f.repo, "develop");
    let commit = merged_commit(GitMerger::new().merge(&spec(&f)));
    assert_merge_commit(&f, &commit, &old);
    assert!(other.join("feature.txt").is_file());
    assert_eq!(git(&other, &["status", "--porcelain"]), "");
    assert!(!f.repo.join("feature.txt").exists(), "main is untouched");
}

#[test]
fn a_base_nobody_has_checked_out_is_updated_without_touching_any_checkout() {
    let f = setup();
    git(&f.repo, &["checkout", "--quiet", "-b", "main"]);
    let old = tip(&f.repo, "develop");
    std::fs::write(f.repo.join("README.md"), "local edit\n").expect("write");
    let commit = merged_commit(GitMerger::new().merge(&spec(&f)));
    assert_merge_commit(&f, &commit, &old);
    assert_eq!(
        std::fs::read_to_string(f.repo.join("README.md")).expect("read"),
        "local edit\n",
        "a dirty checkout of another branch is left alone"
    );
}

#[test]
fn a_conflict_names_the_files_and_changes_nothing() {
    let f = setup();
    commit_file(&f.ship, "README.md", "ship says hi\n", "ship edit");
    commit_file(&f.repo, "README.md", "base says hi\n", "base edit");
    let old = tip(&f.repo, "develop");
    let outcome = GitMerger::new().merge(&spec(&f)).expect("merge");
    assert_eq!(
        outcome,
        MergeOutcome::Conflict {
            files: vec!["README.md".to_owned()]
        }
    );
    assert_eq!(tip(&f.repo, "develop"), old);
    assert_eq!(git(&f.repo, &["status", "--porcelain"]), "");
}

#[test]
fn a_branch_the_base_already_contains_is_up_to_date() {
    let f = setup();
    let merger = GitMerger::new();
    let commit = merged_commit(merger.merge(&spec(&f)));
    assert_eq!(merger.merge(&spec(&f)), Ok(MergeOutcome::UpToDate));
    assert_eq!(tip(&f.repo, "develop"), commit, "no second merge commit");
}

/// A merger that, on its first `moves` attempts, lets another commit land on the base just
/// before the merger updates it.
fn merger_that_loses_the_race(f: &Fixture, moves: u32) -> (GitMerger, Arc<AtomicU32>) {
    let asked = Arc::new(AtomicU32::new(0));
    let seen = asked.clone();
    let repo = f.repo.clone();
    let hook = move |attempt: u32| {
        seen.fetch_add(1, Ordering::SeqCst);
        if attempt < moves {
            let name = format!("other-{attempt}.txt");
            commit_file(&repo, &name, "other\n", "someone else's commit");
        }
    };
    (GitMerger::with_hook(Box::new(hook)), asked)
}

#[test]
fn a_base_that_moves_while_checked_out_is_merged_again_on_top_of_the_new_tip() {
    let f = setup();
    let (merger, asked) = merger_that_loses_the_race(&f, 1);
    let commit = merged_commit(merger.merge(&spec(&f)));
    assert_eq!(asked.load(Ordering::SeqCst), 2, "one retry");
    let parents = git(&f.repo, &["rev-list", "--parents", "-n", "1", &commit]);
    assert_eq!(parents.split(' ').count(), 3);
    assert!(f.repo.join("other-0.txt").is_file(), "their commit is kept");
    assert!(f.repo.join("feature.txt").is_file());
    assert_eq!(git(&f.repo, &["status", "--porcelain"]), "");
}

#[test]
fn a_base_that_moves_while_not_checked_out_fails_the_compare_and_swap_and_retries() {
    let f = setup();
    git(&f.repo, &["checkout", "--quiet", "-b", "main"]);
    // Another commit on develop through a second worktree, so the base stays unchecked out in
    // the repository itself.
    let other = f.repo.parent().expect("parent").join("mover");
    git(
        &f.repo,
        &[
            "worktree",
            "add",
            "--detach",
            &other.to_string_lossy(),
            "develop",
        ],
    );
    let asked = Arc::new(AtomicU32::new(0));
    let seen = asked.clone();
    let (mover, repo) = (other.clone(), f.repo.clone());
    let hook = move |attempt: u32| {
        seen.fetch_add(1, Ordering::SeqCst);
        if attempt == 0 {
            commit_file(&mover, "other.txt", "other\n", "someone else's commit");
            let head = git(&mover, &["rev-parse", "HEAD"]);
            git(&repo, &["update-ref", "refs/heads/develop", &head]);
        }
    };
    let commit = merged_commit(GitMerger::with_hook(Box::new(hook)).merge(&spec(&f)));
    assert_eq!(asked.load(Ordering::SeqCst), 2);
    let files = git(&f.repo, &["ls-tree", "-r", "--name-only", &commit]);
    assert!(
        files.contains("other.txt") && files.contains("feature.txt"),
        "{files}"
    );
}

#[test]
fn a_base_that_never_stops_moving_is_a_typed_failure_and_changes_nothing_of_ours() {
    let f = setup();
    let (merger, asked) = merger_that_loses_the_race(&f, u32::MAX);
    let outcome = merger.merge(&spec(&f));
    assert_eq!(outcome, Err(MergeError::BaseMoved("develop".to_owned())));
    assert_eq!(asked.load(Ordering::SeqCst), 3, "three attempts");
    assert!(!f.repo.join("feature.txt").exists());
}

#[test]
fn missing_branches_and_a_missing_program_are_typed_errors() {
    let f = setup();
    let mut s = spec(&f);
    s.branch = "ship/gone".to_owned();
    assert_eq!(
        GitMerger::new().merge(&s),
        Err(MergeError::BranchMissing("ship/gone".to_owned()))
    );
    let mut s = spec(&f);
    s.base = "nope".to_owned();
    assert_eq!(
        GitMerger::new().merge(&s),
        Err(MergeError::BaseMissing("nope".to_owned()))
    );
    assert_eq!(
        GitMerger::with_program("toknila-no-such-git").merge(&spec(&f)),
        Err(MergeError::GitMissing)
    );
}

#[test]
fn a_git_older_than_the_merge_needs_is_refused_with_both_versions() {
    assert_eq!(
        check_version("git version 2.37.9"),
        Err(MergeError::GitTooOld {
            found: "2.37".to_owned(),
            needs: "2.38".to_owned()
        })
    );
    assert_eq!(check_version("git version 2.38.0"), Ok(()));
}
