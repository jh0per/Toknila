//! `tok daemon stop` ends the daemon while an agent's permission hook still waits for a person's
//! answer: a request that never completes by itself must not hold the graceful stop open. The
//! real binaries, with the workspace's `fake-agent` (never a real agent).

mod common;

use std::time::{Duration, Instant};

use common::{Tok, pid_alive, repo};

fn wait_until(what: &str, mut ok: impl FnMut() -> bool) {
    let until = Instant::now() + Duration::from_secs(30);
    while !ok() {
        assert!(Instant::now() < until, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn stop_ends_the_daemon_while_a_permission_request_is_open() {
    let dir = common::tempdir();
    let tok = Tok::in_dir(dir.path());
    tok.profile(
        "hooked",
        &[
            "post",
            "/tickets/{ticket}/activity",
            r#"{"signal":"hook:pre_tool_use","tool":"Bash: cargo test"}"#,
            "post",
            "/tickets/{ticket}/permission",
            r#"{}"#,
            "sleep",
            "60000",
        ],
        "",
    );
    let repo = repo(dir.path());
    tok.run(&[
        "project",
        "new",
        "DEMO",
        "--repo",
        &repo.to_string_lossy(),
        "--profile",
        "hooked",
    ]);
    tok.run(&["chain", "new", "demo"]);
    tok.run(&["ticket", "new", "demo", "Alpha"]);
    let pid = tok.info().pid;

    // Control: the hook's request is open (the card waits for an answer) when the daemon stops.
    wait_until("the card to wait on the hook", || {
        tok.status("DEMO-1") == "waiting"
    });

    let stop_at = Instant::now();
    tok.raw(&["daemon", "stop"]);
    // CONNECTIONS_GRACE is 1 s; the margin covers effect tasks and a loaded machine.
    let until = stop_at + Duration::from_secs(8);
    while pid_alive(pid) && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(
        !pid_alive(pid),
        "the daemon was still running {:?} after stop with a permission request open",
        stop_at.elapsed()
    );
}
