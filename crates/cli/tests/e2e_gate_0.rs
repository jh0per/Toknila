//! P0-14, gate 0: the whole phase-0 scenario through the real `tok` binary and daemon, with
//! simulated agents. The `--json` output people and scripts read is pinned by snapshots in
//! `tests/snapshots/`; `TOKNILA_UPDATE_SNAPSHOTS=1` rewrites them after an intended change.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

const DEMO: &str = include_str!("../../../examples/demo.toml");

struct Tok {
    home: PathBuf,
}

impl Tok {
    fn run(&self, args: &[&str]) -> Value {
        let out = Command::new(env!("CARGO_BIN_EXE_tok"))
            .args(args)
            .arg("--json")
            .env("TOKNILA_HOME", &self.home)
            .output()
            .expect("run tok");
        assert!(
            out.status.success(),
            "tok {args:?}: {}\n{}",
            out.status,
            String::from_utf8_lossy(&out.stderr)
        );
        if out.stdout.is_empty() {
            return Value::Null;
        }
        serde_json::from_slice(&out.stdout).unwrap_or_else(|e| {
            panic!(
                "tok {args:?} printed non-JSON ({e}): {}",
                String::from_utf8_lossy(&out.stdout)
            )
        })
    }

    fn fails(&self, args: &[&str]) -> String {
        let out = Command::new(env!("CARGO_BIN_EXE_tok"))
            .args(args)
            .env("TOKNILA_HOME", &self.home)
            .output()
            .expect("run tok");
        assert!(!out.status.success(), "tok {args:?} should fail");
        String::from_utf8_lossy(&out.stderr).into_owned()
    }

    fn status(&self, key: &str) -> String {
        self.run(&["show", key])["ticket"]["status"]
            .as_str()
            .expect("status")
            .to_owned()
    }

    fn statuses(&self) -> Vec<String> {
        self.run(&["ls"])
            .as_array()
            .expect("ls is a list")
            .iter()
            .map(|t| {
                format!(
                    "{} {}",
                    t["ticket"]["key"].as_str().unwrap_or("?"),
                    t["ticket"]["status"].as_str().unwrap_or("?")
                )
            })
            .collect()
    }

    fn chain(&self) -> Value {
        self.run(&["chain", "show", "demo"])
    }
}

impl Drop for Tok {
    fn drop(&mut self) {
        let _ = Command::new(env!("CARGO_BIN_EXE_tok"))
            .args(["daemon", "stop"])
            .env("TOKNILA_HOME", &self.home)
            .output();
    }
}

/// Replaces what differs between runs (ids, clocks) with fixed values, keeping every key.
fn normalize(v: &mut Value) {
    match v {
        Value::Object(m) => {
            for (k, x) in m.iter_mut() {
                let clock = k.ends_with("_at")
                    || k.ends_with("_since")
                    || k.ends_with("_ms")
                    || k == "since"
                    || k == "at";
                if clock && x.is_number() {
                    *x = Value::from(0);
                } else {
                    normalize(x);
                }
            }
        }
        Value::Array(a) => a.iter_mut().for_each(normalize),
        Value::String(s)
            if s.len() == 26
                && s.bytes()
                    .all(|b| b.is_ascii_digit() || b.is_ascii_uppercase()) =>
        {
            *s = "<id>".to_owned();
        }
        _ => {}
    }
}

fn snapshot(name: &str, mut v: Value) {
    normalize(&mut v);
    let text = format!("{}\n", serde_json::to_string_pretty(&v).expect("json"));
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/snapshots")
        .join(format!("{name}.json"));
    if std::env::var_os("TOKNILA_UPDATE_SNAPSHOTS").is_some() {
        std::fs::write(&path, &text).expect("write snapshot");
        return;
    }
    let want = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| {
            panic!(
                "missing snapshot {}; run with TOKNILA_UPDATE_SNAPSHOTS=1",
                path.display()
            )
        })
        .replace("\r\n", "\n");
    assert_eq!(
        text, want,
        "snapshot {name} changed; if intended, rerun with TOKNILA_UPDATE_SNAPSHOTS=1"
    );
}

#[test]
fn gate_0() {
    let dir = tempfile::tempdir().expect("tempdir");
    let tok = Tok {
        home: dir.path().join("home"),
    };
    let file = dir.path().join("demo.toml");
    std::fs::write(&file, DEMO).expect("write demo");
    let file = file.to_str().expect("utf-8 path");

    // 1. Plan: a project and the chain from a file; autostart picks up the first ticket.
    tok.run(&["project", "new", "DEMO", "--name", "Demo"]);
    tok.run(&["chain", "import", file, "--project", "DEMO"]);
    assert_eq!(
        tok.statuses(),
        [
            "DEMO-1 running",
            "DEMO-2 backlog",
            "DEMO-3 backlog",
            "DEMO-4 backlog"
        ]
    );
    assert_eq!(tok.chain()["summary"]["status"], "running");
    snapshot("chain_show_imported", tok.chain());

    // 2. The agent works through its checklist and hands in; a person returns it once.
    tok.run(&["sim", "DEMO-1", "subtask", "1", "done"]);
    tok.run(&["sim", "DEMO-1", "subtask", "2", "done"]);
    tok.run(&["sim", "DEMO-1", "finish", "A is done"]);
    assert_eq!(tok.status("DEMO-1"), "review");
    let human = tok.run(&["ls", "--needs-human"]);
    assert_eq!(human.as_array().map(Vec::len), Some(1), "{human}");
    assert_eq!(human[0]["ticket"]["key"], "DEMO-1");
    tok.run(&["return", "DEMO-1", "add a test"]);
    assert_eq!(tok.status("DEMO-1"), "running");
    tok.run(&["sim", "DEMO-1", "finish", "A with a test"]);
    tok.run(&["accept", "DEMO-1"]);
    assert_eq!(
        tok.statuses(),
        [
            "DEMO-1 merged",
            "DEMO-2 running",
            "DEMO-3 running",
            "DEMO-4 backlog"
        ]
    );

    // 3. An agent asks; the chain shows where it stopped; the answer resumes it.
    tok.run(&["sim", "DEMO-2", "ask", "which way?"]);
    let c = tok.chain();
    assert_eq!(c["summary"]["status"], "needs_human");
    assert_eq!(c["summary"]["stopped_at"]["ticket"], "DEMO-2");
    assert_eq!(c["summary"]["stopped_at"]["reason"]["text"], "which way?");
    tok.run(&["comment", "DEMO-2", "left"]);
    assert_eq!(
        tok.status("DEMO-2"),
        "running",
        "the sim agent reads the instruction at once"
    );

    // 4. An agent crashes; a person unblocks it; it returns to where it was.
    tok.run(&["sim", "DEMO-3", "crash", "out of memory"]);
    assert_eq!(tok.status("DEMO-3"), "blocked");
    assert_eq!(tok.chain()["summary"]["status"], "blocked");
    tok.run(&["unblock", "DEMO-3"]);
    assert_eq!(tok.status("DEMO-3"), "running");

    // 5. Both hand in and are accepted; the manual milestone holds the rest.
    tok.run(&["sim", "DEMO-2", "finish", "B is done"]);
    tok.run(&["sim", "DEMO-3", "finish", "D is done"]);
    tok.run(&["accept", "DEMO-2"]);
    tok.run(&["accept", "DEMO-3"]);
    let c = tok.chain();
    assert_eq!(c["summary"]["status"], "idle");
    assert_eq!(c["summary"]["next_release"]["version"], "R1");
    assert_eq!(c["summary"]["next_release"]["state"], "planned");
    assert_eq!(tok.status("DEMO-4"), "backlog");
    snapshot("show_ticket", tok.run(&["show", "DEMO-2"]));

    // 6. The release: cut starts what waits for it; the release agent finishes it.
    let msg = tok.fails(&["accept", "DEMO-4"]);
    assert!(
        msg.contains("cannot accept a ticket in Backlog"),
        "accepting a Backlog ticket is refused: {msg}"
    );
    tok.run(&["release", "start", "R1"]);
    assert_eq!(tok.status("DEMO-4"), "running");
    tok.run(&["sim", "R1", "finish"]);
    assert_eq!(
        tok.statuses(),
        [
            "DEMO-1 released",
            "DEMO-2 released",
            "DEMO-3 released",
            "DEMO-4 running"
        ]
    );
    tok.run(&["sim", "DEMO-4", "finish", "C is done"]);
    tok.run(&["accept", "DEMO-4"]);
    let c = tok.chain();
    assert_eq!(c["summary"]["done"], 4);
    assert_eq!(c["summary"]["released"], 3);
    snapshot("chain_show_final", c);
    snapshot("ls_final", tok.run(&["ls"]));

    // 7. The chain goes out as a file and comes back as the same graph.
    let out = dir.path().join("out.toml");
    tok.run(&["chain", "export", "demo", out.to_str().expect("utf-8 path")]);
    let exported = std::fs::read_to_string(&out).expect("exported file");
    tok.run(&["project", "new", "COPY"]);
    tok.run(&[
        "chain",
        "import",
        out.to_str().expect("utf-8 path"),
        "--project",
        "COPY",
    ]);
    let copy = tok.run(&["chain", "ls"]);
    assert_eq!(copy.as_array().map(Vec::len), Some(2), "{copy}");
    assert!(
        exported.contains("after = [\"DEMO-2\", \"DEMO-3\"]"),
        "{exported}"
    );
}
