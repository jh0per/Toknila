//! A daemon started with a lifeline dies with the process that started it, even by SIGKILL.

mod common;

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use common::{DaemonGuard, daemon_healthy, kill_hard, pid_alive, start_daemon};
use toknila_daemon::info::DaemonInfo;

const HOME_VAR: &str = "TOKNILA_TEST_PARENT_HOME";

/// Runs only as the child of `daemon_dies_when_its_parent_is_killed`: starts a daemon and waits to
/// be killed.
#[test]
fn parent_that_starts_a_daemon_and_waits() {
    let Some(home) = std::env::var_os(HOME_VAR) else {
        return;
    };
    start_daemon(std::path::Path::new(&home));
    loop {
        std::thread::sleep(Duration::from_secs(1));
    }
}

#[test]
fn daemon_dies_when_its_parent_is_killed() {
    let dir = common::tempdir();
    let home = dir.path().join("home");
    // Safety net only: a failed assertion must not leave the daemon behind.
    let _stop = DaemonGuard::new(&home);

    let mut parent = Command::new(std::env::current_exe().expect("test binary"))
        .args([
            "--exact",
            "parent_that_starts_a_daemon_and_waits",
            "--nocapture",
        ])
        .env(HOME_VAR, &home)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn the parent");

    let until = Instant::now() + Duration::from_secs(60);
    while !daemon_healthy(&home) {
        assert!(Instant::now() < until, "the daemon did not come up");
        std::thread::sleep(Duration::from_millis(50));
    }
    let pid = DaemonInfo::read(&home.join("daemon.json"))
        .expect("daemon.json")
        .expect("daemon runs")
        .pid;
    // Control: the daemon lives as long as its parent does.
    std::thread::sleep(Duration::from_millis(500));
    assert!(pid_alive(pid), "the daemon must outlive nothing yet");

    kill_hard(parent.id());
    let _ = parent.wait();

    let until = Instant::now() + Duration::from_secs(10);
    while pid_alive(pid) && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(
        !pid_alive(pid),
        "daemon {pid} still runs 10 s after its parent was killed"
    );
}
