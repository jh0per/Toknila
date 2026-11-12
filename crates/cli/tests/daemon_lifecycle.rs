//! `tok daemon start / status / stop` against the real binary, in a private TOKNILA_HOME.

use std::process::Command;

use serde_json::Value;

mod common;

fn tok(home: &std::path::Path, args: &[&str]) -> Value {
    let out = if args == ["daemon", "start"] {
        // The daemon is started with a lifeline, so it dies with this test binary.
        common::start_daemon(home)
    } else {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_tok"));
        cmd.args(args).arg("--json").env("TOKNILA_HOME", home);
        common::isolate(&mut cmd, home);
        cmd.output().expect("run tok")
    };
    assert!(
        out.status.success(),
        "tok {args:?} failed: {}\n{}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).unwrap_or_else(|e| {
        panic!(
            "tok {args:?} printed non-JSON ({e}): {}",
            String::from_utf8_lossy(&out.stdout)
        )
    })
}

fn status_code(r: Result<ureq::Response, ureq::Error>) -> u16 {
    match r {
        Ok(resp) => resp.status(),
        Err(ureq::Error::Status(code, _)) => code,
        Err(e) => panic!("transport error: {e}"),
    }
}

#[test]
fn start_status_auth_stop() {
    let home = common::tempdir();
    let h = home.path();
    // Stops the daemon even when an assertion fails or the run is interrupted.
    let _stop = common::DaemonGuard::new(h);

    assert_eq!(tok(h, &["daemon", "status"])["running"], false);

    let started = tok(h, &["daemon", "start"]);
    assert_eq!(started["running"], true, "{started}");
    let port = started["port"].as_u64().expect("port");
    assert!(h.join("daemon.json").exists());
    assert!(h.join("toknila.db").exists());

    // Starting again is a no-op that reports the same daemon.
    assert_eq!(tok(h, &["daemon", "start"])["pid"], started["pid"]);
    assert_eq!(tok(h, &["daemon", "status"])["port"], port);
    // The desktop app reads this to decide whether replacing the daemon needs asking.
    assert_eq!(tok(h, &["daemon", "status"])["agents"], 0);

    let base = format!("http://127.0.0.1:{port}");
    assert_eq!(
        status_code(ureq::get(&format!("{base}/healthz")).call()),
        200
    );
    assert_eq!(
        status_code(ureq::post(&format!("{base}/v1/shutdown")).call()),
        401
    );

    assert_eq!(tok(h, &["daemon", "stop"])["stopped"], true);
    assert_eq!(tok(h, &["daemon", "status"])["running"], false);
    // The daemon removes its file as it exits; `stop` may see the daemon gone a moment before the
    // removal shows, so the claim is "it goes", awaited with a deadline.
    let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while h.join("daemon.json").exists() && std::time::Instant::now() < until {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(
        !h.join("daemon.json").exists(),
        "clean stop removes daemon.json"
    );
    assert_eq!(tok(h, &["daemon", "stop"])["stopped"], false);

    // A restart takes the same address and token, so an open web page reconnects by itself.
    let token = |h: &std::path::Path| -> String {
        let v: Value = serde_json::from_str(
            &std::fs::read_to_string(h.join("daemon.json")).expect("daemon.json"),
        )
        .expect("json");
        v["token"].as_str().expect("token").to_owned()
    };
    let last: Value = serde_json::from_str(
        &std::fs::read_to_string(h.join("daemon.last.json")).expect("clean stop keeps the address"),
    )
    .expect("json");
    let again = tok(h, &["daemon", "start"]);
    assert_eq!(again["port"], port, "same port after restart");
    assert_eq!(token(h), last["token"].as_str().expect("token"));
    assert_eq!(tok(h, &["daemon", "stop"])["stopped"], true);

    let logs: Vec<_> = std::fs::read_dir(h.join("logs"))
        .expect("logs dir")
        .collect();
    assert!(!logs.is_empty(), "daemon wrote a log file");
}

#[test]
fn a_cut_address_file_is_reported_as_incomplete_never_as_an_os_error() {
    let home = common::tempdir();
    let h = home.path();
    for text in ["", "{\"pid\":1,\"po"] {
        std::fs::write(h.join("daemon.json"), text).expect("cut file");
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_tok"));
        cmd.args(["daemon", "status"]).env("TOKNILA_HOME", h);
        common::isolate(&mut cmd, h);
        let out = cmd.output().expect("run tok");
        assert!(!out.status.success(), "status of a cut file succeeds");
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(
            err.contains("the address file is incomplete"),
            "stderr: {err}"
        );
        assert!(!err.contains("os error"), "stderr: {err}");
    }
}

#[test]
fn an_address_file_still_being_written_is_waited_for() {
    let home = common::tempdir();
    let h = home.path().to_path_buf();
    let _stop = common::DaemonGuard::new(&h);
    let started = tok(&h, &["daemon", "start"]);
    let full = std::fs::read_to_string(h.join("daemon.json")).expect("daemon.json");
    std::fs::write(h.join("daemon.json"), "").expect("empty for a moment");
    let (path, text) = (h.join("daemon.json"), full);
    let writer = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(150));
        std::fs::write(path, text).expect("restore");
    });
    let status = tok(&h, &["daemon", "status"]);
    writer.join().expect("writer");
    assert_eq!(status["running"], true, "{status}");
    assert_eq!(status["pid"], started["pid"]);
}
