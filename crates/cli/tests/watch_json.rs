//! `tok watch --json` prints one JSON object per journal entry, not the text format.

mod common;

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use common::{Tok, snapshot};

#[test]
fn watch_json_prints_each_journal_entry_as_one_json_line() {
    let dir = tempfile::tempdir().expect("tempdir");
    let tok = Tok::in_dir(dir.path());
    tok.run(&["project", "new", "DEMO"]);

    let mut child = Command::new(env!("CARGO_BIN_EXE_tok"))
        .args(["watch", "--json", "--since", "0"])
        .env("TOKNILA_HOME", &tok.home)
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn tok watch");
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
    snapshot("watch_json_first_entry", entry);
}
