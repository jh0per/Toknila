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
    assert!(started.elapsed() < Duration::from_secs(60), "took too long");
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

/// A stand-in for the program that starts a sleeping helper, as `git-remote-https` is, and waits.
#[cfg(unix)]
#[test]
fn a_timeout_ends_the_helper_processes_too() {
    use std::os::unix::fs::PermissionsExt;
    let tmp = tempfile::tempdir().expect("tmp");
    let pid_file = tmp.path().join("helper.pid");
    let script = tmp.path().join("fake-git");
    std::fs::write(
        &script,
        format!(
            "#!/bin/sh\nsleep 60 &\necho $! > '{}'\nwait\n",
            pid_file.display()
        ),
    )
    .expect("script");
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    let g = Git::new(&script.to_string_lossy());
    // The limit must pass after the script has started its helper: on a loaded machine starting a
    // shell can take seconds, and a kill before that leaves no helper to look for. The limit is
    // this test's own time, so it is generous but the pid file is awaited, not assumed.
    let err = g
        .run_within("/", &["fetch"], Duration::from_secs(8))
        .expect_err("times out");
    assert!(matches!(err, GitError::Failed { .. }), "{err:?}");
    let pid: i32 = std::fs::read_to_string(&pid_file)
        .expect("the script started its helper within the limit")
        .trim()
        .parse()
        .expect("number");
    let alive = || nix::sys::signal::kill(nix::unistd::Pid::from_raw(pid), None).is_ok();
    // The helper's end is awaited, with a ceiling far above what it takes.
    let until = Instant::now() + Duration::from_secs(60);
    while alive() && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(!alive(), "the helper outlived the timeout");
}
