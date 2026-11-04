//! Helpers shared by the end-to-end tests: the real `tok` binary against a temporary data
//! folder, and JSON snapshots in `tests/snapshots/` (`TOKNILA_UPDATE_SNAPSHOTS=1` rewrites them
//! after an intended change).

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

/// The `tok` binary of this build, run against one data folder.
pub struct Tok {
    pub home: PathBuf,
}

impl Tok {
    /// Runs `tok <args> --json` and parses what it prints.
    pub fn run(&self, args: &[&str]) -> Value {
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

    /// Runs `tok <args>`, expects it to fail and returns its stderr.
    pub fn fails(&self, args: &[&str]) -> String {
        let out = Command::new(env!("CARGO_BIN_EXE_tok"))
            .args(args)
            .env("TOKNILA_HOME", &self.home)
            .output()
            .expect("run tok");
        assert!(!out.status.success(), "tok {args:?} should fail");
        String::from_utf8_lossy(&out.stderr).into_owned()
    }

    /// The status of one ticket.
    pub fn status(&self, key: &str) -> String {
        self.run(&["show", key])["ticket"]["status"]
            .as_str()
            .expect("status")
            .to_owned()
    }

    /// `KEY status` of every ticket, in key order.
    pub fn statuses(&self) -> Vec<String> {
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

    pub fn chain(&self) -> Value {
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

/// Compares `v`, normalized, with `tests/snapshots/<name>.json`.
pub fn snapshot(name: &str, mut v: Value) {
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
