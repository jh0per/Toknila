//! `tok daemon start / status / stop` against the real binary, in a private TOKNILA_HOME.

use std::process::Command;

use serde_json::Value;

mod common;

fn tok(home: &std::path::Path, args: &[&str]) -> Value {
    let out = Command::new(env!("CARGO_BIN_EXE_tok"))
        .args(args)
        .arg("--json")
        .env("TOKNILA_HOME", home)
        .output()
        .expect("run tok");
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
    let home = tempfile::tempdir().expect("tempdir");
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
