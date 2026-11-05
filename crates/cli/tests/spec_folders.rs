//! P1-04: a spec folder through the real `tok` binary and daemon: import, tick a subtask as the
//! simulated agent, export into a copy, check the file. `TOKNILA_UPDATE_SNAPSHOTS=1` rewrites
//! the snapshot after an intended change.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;

const FOLDER: &str = "001-sample-feature";

struct Tok {
    home: PathBuf,
}

impl Tok {
    fn raw(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_tok"))
            .args(args)
            .env("TOKNILA_HOME", &self.home)
            .output()
            .expect("run tok")
    }

    fn json(&self, args: &[&str]) -> Value {
        let mut all = args.to_vec();
        all.push("--json");
        let out = self.raw(&all);
        assert!(
            out.status.success(),
            "tok {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_slice(&out.stdout).expect("json")
    }

    fn ok(&self, args: &[&str]) -> String {
        let out = self.raw(args);
        assert!(
            out.status.success(),
            "tok {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stderr).into_owned()
    }

    fn statuses(&self) -> Vec<String> {
        let list = self.json(&["ls"]);
        let item = |t: &Value| {
            format!(
                "{} {}",
                t["ticket"]["key"].as_str().unwrap_or("?"),
                t["ticket"]["status"].as_str().unwrap_or("?")
            )
        };
        list.as_array().expect("list").iter().map(item).collect()
    }
}

impl Drop for Tok {
    fn drop(&mut self) {
        let _ = self.raw(&["daemon", "stop"]);
    }
}

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
        .unwrap_or_else(|_| panic!("missing snapshot {}", path.display()))
        .replace("\r\n", "\n");
    assert_eq!(text, want, "snapshot {name} changed");
}

#[test]
fn spec_folder_round_trip() {
    let dir = tempfile::tempdir().expect("tempdir");
    let tok = Tok {
        home: dir.path().join("home"),
    };
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

    tok.json(&["project", "new", "SPEC", "--name", "Spec"]);
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
    let chain = tok.json(&["chain", "show", FOLDER]);
    assert_eq!(chain["chain"]["autostart"], false);
    snapshot("spec_import_chain_show", chain);
    let story = tok.json(&["show", "SPEC-3"]);
    assert_eq!(story["subtasks"][0]["done"], true);
    assert_eq!(story["subtasks"][1]["done"], false);

    // Export right after import: byte-identical, and the file is not even rewritten.
    let copy = dir.path().join("copy");
    copy_dir(&specs.join(FOLDER), &copy);
    let copy_arg = copy.to_str().expect("utf-8 path");
    tok.ok(&["chain", "export", FOLDER, "--spec", copy_arg]);
    assert_eq!(
        std::fs::read(copy.join("tasks.md")).expect("read"),
        original
    );

    // The agent ticks one subtask; export changes exactly that line.
    tok.ok(&["sim", "SPEC-3", "start"]);
    tok.ok(&["sim", "SPEC-3", "subtask", "2", "done"]);
    tok.ok(&["chain", "export", FOLDER, "--spec", copy_arg]);
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
    tok.ok(&["chain", "export", FOLDER, "--spec", copy_arg]);
    assert_eq!(std::fs::read(copy.join("tasks.md")).expect("read"), patched);

    // A proposed wildcard card is not written; once approved it is appended, numbered after the
    // highest T-id, and the original bytes before it stay.
    tok.ok(&[
        "sim",
        "SPEC-3",
        "propose",
        "Token refresh",
        "--after",
        "SPEC-3",
    ]);
    tok.ok(&["subtask", "add", "SPEC-7", "write the endpoint"]);
    tok.ok(&["chain", "export", FOLDER, "--spec", copy_arg]);
    assert_eq!(std::fs::read(copy.join("tasks.md")).expect("read"), patched);
    tok.ok(&["approve", "SPEC-7"]);
    tok.ok(&["chain", "export", FOLDER, "--spec", copy_arg]);
    let appended = std::fs::read(copy.join("tasks.md")).expect("read");
    assert!(appended.starts_with(&patched));
    assert_eq!(
        String::from_utf8_lossy(&appended[patched.len()..]),
        "\n## Phase 7: Token refresh\n\n- [ ] T012 write the endpoint\n"
    );
    tok.ok(&["chain", "export", FOLDER, "--spec", copy_arg]);
    assert_eq!(
        std::fs::read(copy.join("tasks.md")).expect("read"),
        appended
    );
    assert_eq!(
        std::fs::read(specs.join(FOLDER).join("tasks.md")).expect("read"),
        original
    );
    assert_eq!(
        std::fs::read(specs.join(FOLDER).join("tasks.md")).expect("read"),
        original
    );
}
