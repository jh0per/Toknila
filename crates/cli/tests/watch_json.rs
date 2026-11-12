//! `tok watch --json` prints one JSON object per journal entry, not the text format.

mod common;

use std::io::{BufRead, BufReader};
use std::sync::mpsc;
use std::time::Duration;

use common::{Tok, snapshot, spawn_watch};

#[test]
fn watch_json_prints_each_journal_entry_as_one_json_line() {
    let dir = common::tempdir();
    let tok = Tok::in_dir(dir.path());
    tok.run(&["project", "new", "DEMO", "--sim"]);

    let mut child = spawn_watch(&tok.home, &["--json", "--since", "0"]);
    let out = child.stdout.take().expect("stdout");
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(out).lines().map_while(Result::ok) {
            let _ = tx.send(line);
        }
    });
    let first = rx.recv_timeout(Duration::from_secs(30));
    let _ = child.kill();
    let _ = child.wait();
    let line = first.expect("a journal line");
    let entry: serde_json::Value = serde_json::from_str(&line)
        .unwrap_or_else(|e| panic!("watch --json printed non-JSON ({e}): {line}"));
    assert_eq!(entry["entity"], "project");
    assert_eq!(entry["kind"], "created");
    // The event time is real (epoch ms, near now); the snapshot below zeroes it.
    let at = entry["at"].as_i64().expect("at is epoch milliseconds");
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_millis() as i64;
    assert!((now - at).abs() < 10 * 60 * 1000, "at={at} now={now}");
    snapshot("watch_json_first_entry", entry);
}

#[test]
fn watch_plain_line_starts_with_the_local_time() {
    let dir = common::tempdir();
    let tok = Tok::in_dir(dir.path());
    tok.run(&["project", "new", "DEMO", "--sim"]);

    let mut child = spawn_watch(&tok.home, &["--since", "0"]);
    let out = child.stdout.take().expect("stdout");
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(out).lines().map_while(Result::ok) {
            let _ = tx.send(line);
        }
    });
    let first = rx.recv_timeout(Duration::from_secs(30));
    let _ = child.kill();
    let _ = child.wait();
    let line = first.expect("a journal line");
    let (time, rest) = line.split_once(' ').expect("time then text");
    let digits: Vec<&str> = time.split(':').collect();
    assert!(
        digits.len() == 3
            && digits
                .iter()
                .all(|d| d.len() == 2 && d.bytes().all(|b| b.is_ascii_digit())),
        "line must start with HH:MM:SS: {line}"
    );
    // The clock time varies with the run and the zone; the rest is pinned.
    snapshot(
        "watch_plain_first_line",
        serde_json::Value::String(format!("<HH:MM:SS> {rest}")),
    );
}
