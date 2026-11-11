//! Gate 0: the whole phase-0 scenario through the real `tok` binary and daemon, with
//! simulated agents. The `--json` output people and scripts read is pinned by snapshots in
//! `tests/snapshots/`; `TOKNILA_UPDATE_SNAPSHOTS=1` rewrites them after an intended change.

mod common;

use common::{Tok, snapshot};

const DEMO: &str = include_str!("../../../examples/demo.toml");

#[test]
fn gate_0() {
    let dir = common::tempdir();
    let tok = Tok::in_dir(dir.path());
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
    // The daemon read the file, and the chain remembers where from.
    let source = tok.chain()["chain"]["source"].clone();
    assert!(
        source.as_str().is_some_and(|s| s.ends_with("demo.toml")),
        "{source}"
    );
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
