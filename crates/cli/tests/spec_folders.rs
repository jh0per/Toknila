//! A spec folder through the real `tok` binary and daemon: import, tick a subtask as the
//! simulated agent, export into a copy, check the file. `TOKNILA_UPDATE_SNAPSHOTS=1` rewrites
//! the snapshot after an intended change.

mod common;

use std::path::{Path, PathBuf};

use common::{Tok, snapshot};
use serde_json::Value;

const FOLDER: &str = "001-sample-feature";

fn examples() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/specs")
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("mkdir");
    for e in std::fs::read_dir(from).expect("read_dir") {
        let e = e.expect("entry");
        let target = to.join(e.file_name());
        if e.path().is_dir() {
            copy_dir(&e.path(), &target);
        } else {
            std::fs::write(&target, std::fs::read(e.path()).expect("read")).expect("write");
        }
    }
}

fn lines(bytes: &[u8]) -> Vec<String> {
    let text = String::from_utf8(bytes.to_vec()).expect("utf-8");
    text.split_inclusive('\n').map(str::to_owned).collect()
}

#[test]
fn spec_folder_round_trip() {
    let dir = tempfile::tempdir().expect("tempdir");
    let tok = Tok::in_dir(dir.path());
    let specs = dir.path().join("specs");
    copy_dir(&examples().join(FOLDER), &specs.join(FOLDER));
    let original = std::fs::read(specs.join(FOLDER).join("tasks.md")).expect("tasks.md");
    // A folder without tasks.md is skipped; one with a malformed line fails and names its line.
    std::fs::create_dir_all(specs.join("002-not-tasked")).expect("mkdir");
    std::fs::create_dir_all(specs.join("003-broken")).expect("mkdir");
    std::fs::write(
        specs.join("003-broken/tasks.md"),
        "## Phase 1: A\n- [ ] T001 ok\n- [q] T002 bad\n",
    )
    .expect("write");
    let specs_arg = specs.to_str().expect("utf-8 path");

    tok.run(&["project", "new", "SPEC", "--name", "Spec"]);
    let out = tok.raw(&["chain", "import", specs_arg, "--json"]);
    assert!(!out.status.success(), "a failed folder fails the command");
    let report: Value = serde_json::from_slice(&out.stdout).expect("report is json");
    let results: Vec<_> = report
        .as_array()
        .expect("list")
        .iter()
        .map(|o| (o["folder"].as_str().unwrap_or(""), o["result"].as_str()))
        .collect();
    assert_eq!(
        results,
        [
            (FOLDER, Some("imported")),
            ("002-not-tasked", Some("skipped")),
            ("003-broken", Some("failed"))
        ]
    );
    assert_eq!(report[0]["cards"], 6);
    assert_eq!(report[0]["tasks"], 11);
    let error = report[2]["error"].as_str().expect("error text");
    assert!(
        error.ends_with("tasks.md:3: malformed task line: expected `- [ ] text` or `- [x] text`"),
        "{error}"
    );

    // Importing the same folder again is refused with a clear error.
    let again = tok.raw(&[
        "chain",
        "import",
        specs.join(FOLDER).to_str().expect("path"),
    ]);
    assert!(!again.status.success());
    let message = String::from_utf8_lossy(&again.stdout).into_owned()
        + &String::from_utf8_lossy(&again.stderr);
    assert!(
        message.contains("chain 001-sample-feature already exists in SPEC"),
        "{message}"
    );

    // Done phases are Merged through the rules; the stories are Ready (autostart is off), Polish
    // waits for all three.
    assert_eq!(
        tok.statuses(),
        [
            "SPEC-1 merged",
            "SPEC-2 merged",
            "SPEC-3 ready",
            "SPEC-4 ready",
            "SPEC-5 ready",
            "SPEC-6 backlog"
        ]
    );
    let chain = tok.run(&["chain", "show", FOLDER]);
    assert_eq!(chain["chain"]["autostart"], false);
    // The daemon read the folder, and the chain remembers which one.
    let source = chain["chain"]["source"].as_str().expect("source");
    assert!(source.ends_with(FOLDER), "{source}");
    snapshot("spec_import_chain_show", chain);
    let story = tok.run(&["show", "SPEC-3"]);
    assert_eq!(story["subtasks"][0]["done"], true);
    assert_eq!(story["subtasks"][1]["done"], false);

    // Export right after import: byte-identical, and the file is not even rewritten.
    let copy = dir.path().join("copy");
    copy_dir(&specs.join(FOLDER), &copy);
    let copy_arg = copy.to_str().expect("utf-8 path");
    tok.text(&["chain", "export", FOLDER, "--spec", copy_arg]);
    assert_eq!(
        std::fs::read(copy.join("tasks.md")).expect("read"),
        original
    );

    // The agent ticks one subtask; export changes exactly that line.
    tok.text(&["sim", "SPEC-3", "start"]);
    tok.text(&["sim", "SPEC-3", "subtask", "2", "done"]);
    tok.text(&["chain", "export", FOLDER, "--spec", copy_arg]);
    let patched = std::fs::read(copy.join("tasks.md")).expect("read");
    let (before, after) = (lines(&original), lines(&patched));
    assert_eq!(before.len(), after.len());
    let changed: Vec<_> = before.iter().zip(&after).filter(|(a, b)| a != b).collect();
    assert_eq!(changed.len(), 1, "{changed:?}");
    assert_eq!(
        changed[0].0.replacen("[ ]", "[x]", 1),
        *changed[0].1,
        "only the checkbox flipped"
    );
    assert!(changed[0].1.starts_with("- [x] T006 "));

    // Exporting twice gives no further change; the originals of the repo are untouched.
    tok.text(&["chain", "export", FOLDER, "--spec", copy_arg]);
    assert_eq!(std::fs::read(copy.join("tasks.md")).expect("read"), patched);

    // A proposed wildcard card is not written; once approved it is appended, numbered after the
    // highest T-id, and the original bytes before it stay.
    tok.text(&[
        "sim",
        "SPEC-3",
        "propose",
        "Token refresh",
        "--after",
        "SPEC-3",
    ]);
    tok.text(&["subtask", "add", "SPEC-7", "write the endpoint"]);
    tok.text(&["chain", "export", FOLDER, "--spec", copy_arg]);
    assert_eq!(std::fs::read(copy.join("tasks.md")).expect("read"), patched);
    tok.text(&["approve", "SPEC-7"]);
    tok.text(&["chain", "export", FOLDER, "--spec", copy_arg]);
    let appended = std::fs::read(copy.join("tasks.md")).expect("read");
    assert!(appended.starts_with(&patched));
    assert_eq!(
        String::from_utf8_lossy(&appended[patched.len()..]),
        "\n## Phase 7: Token refresh\n\n- [ ] T012 write the endpoint\n"
    );
    tok.text(&["chain", "export", FOLDER, "--spec", copy_arg]);
    assert_eq!(
        std::fs::read(copy.join("tasks.md")).expect("read"),
        appended
    );
    assert_eq!(
        std::fs::read(specs.join(FOLDER).join("tasks.md")).expect("read"),
        original
    );
}

#[test]
fn a_gap_and_early_tasks_import_and_export_unchanged() {
    let dir = tempfile::tempdir().expect("tempdir");
    let tok = Tok::in_dir(dir.path());
    let folder = dir.path().join("specs/010-gap");
    std::fs::create_dir_all(&folder).expect("mkdir");
    std::fs::write(folder.join("spec.md"), "# Gap Feature\n\nWords.\n").expect("spec");
    let tasks = "- [x] T001 early one\n- [x] T002 early two\n\n## Phase 1: Middle\n\n- [x] T003 a\n- [ ] T004 b\n\n## Phase 2: Last\n\n- [x] T005 c\n";
    std::fs::write(folder.join("tasks.md"), tasks).expect("tasks");
    tok.run(&["project", "new", "SPEC", "--name", "Spec"]);

    let text = tok.raw(&["chain", "import", folder.to_str().expect("path")]);
    assert!(text.status.success());
    let out = String::from_utf8_lossy(&text.stdout).into_owned();
    assert!(
        out.contains("\n  phase-2 done out of order: phase-1 has 1 open subtask (T004)\n"),
        "{out}"
    );
    // The early tasks are one card named after the spec, first, with no phase number.
    assert_eq!(
        tok.statuses(),
        ["SPEC-1 merged", "SPEC-2 ready", "SPEC-3 merged"]
    );
    let early = tok.run(&["show", "SPEC-1"]);
    assert_eq!(early["ticket"]["title"], "Gap Feature");
    assert_eq!(early["ticket"].get("done_out_of_order"), None);
    // The mark is part of the ticket detail, with the key of the open card.
    let last = tok.run(&["show", "SPEC-3"]);
    assert_eq!(last["ticket"]["done_out_of_order"]["behind"][0], "SPEC-2");

    let copy = dir.path().join("copy");
    copy_dir(&folder, &copy);
    let copy_arg = copy.to_str().expect("path");
    tok.text(&["chain", "export", "010-gap", "--spec", copy_arg]);
    assert_eq!(
        std::fs::read_to_string(copy.join("tasks.md")).expect("read"),
        tasks
    );
}
