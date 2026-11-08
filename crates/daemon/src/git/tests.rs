//! The bounded `git` runs: a remote that never answers must not hold a start.

use std::net::TcpListener;
use std::time::{Duration, Instant};

use super::testing::git;
use super::*;

fn repo(dir: &std::path::Path) -> String {
    git(dir, &["init", "--quiet"]);
    dir.to_string_lossy().into_owned()
}

#[test]
fn a_fetch_from_a_remote_that_never_answers_is_given_up_at_the_limit() {
    // Accepts connections and says nothing, as a network that hangs does.
    let silent = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = silent.local_addr().expect("addr").port();
    let tmp = tempfile::tempdir().expect("tmp");
    let repo = repo(tmp.path());
    let url = format!("http://127.0.0.1:{port}/never.git");
    let started = Instant::now();
    let err = Git::new("git")
        .fetch_within(&repo, &[url.as_str()], Duration::from_secs(1))
        .expect_err("gives up");
    assert!(started.elapsed() < Duration::from_secs(15), "took too long");
    let GitError::Failed { command, detail } = err else {
        panic!("not a failure: {err:?}");
    };
    assert_eq!(detail, "timed out after 1 s");
    assert!(command.starts_with("fetch "), "{command}");
    drop(silent);
}

#[test]
fn a_command_within_the_limit_gives_its_output_and_its_failure() {
    let tmp = tempfile::tempdir().expect("tmp");
    let repo = repo(tmp.path());
    let g = Git::new("git");
    let limit = Duration::from_secs(20);
    assert_eq!(
        g.run_within(&repo, &["rev-parse", "--is-inside-work-tree"], limit),
        Ok("true".to_owned())
    );
    let err = g
        .run_within(&repo, &["rev-parse", "--verify", "nope"], limit)
        .expect_err("fails");
    let GitError::Failed { detail, .. } = err else {
        panic!("{err:?}");
    };
    assert!(detail.starts_with("fatal:"), "{detail}");
}
