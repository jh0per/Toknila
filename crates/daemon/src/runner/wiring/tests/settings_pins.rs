//! The settings file Claude Code reads, pinned per role (`snapshots/<role>-settings.json`): a
//! card agent's is what it was before a planner got hooks, byte for byte except the deny rules for
//! the private folders of this machine's home, which are checked by value and then stand as one
//! marker, so the snapshot reads the same on every machine.

use std::path::Path;

use serde_json::Value;
use toknila_core::{AgentProfile, AgentRole};

use super::{DENIED, Rig, denied_with_private};

/// What stands in the pinned file for the rules of this home's private folders.
const PRIVATE: &str = "<deny rules for the private folders of the home>";

/// `text` with the private folders' rules replaced by [`PRIVATE`], after checking that they are
/// exactly this home's and that the file is serde_json's pretty form of itself: what is compared
/// is then the file's bytes, apart from those rules.
fn pinned_form(text: &str) -> String {
    let mut v: Value = serde_json::from_str(text).expect("json");
    assert_eq!(
        serde_json::to_string_pretty(&v).expect("json"),
        text,
        "the file is written in serde_json's pretty form"
    );
    assert_eq!(
        v["permissions"]["deny"],
        serde_json::json!(denied_with_private()),
        "{text}"
    );
    let mut deny: Vec<Value> = DENIED.iter().copied().map(Value::from).collect();
    deny.push(Value::from(PRIVATE));
    v["permissions"]["deny"] = Value::Array(deny);
    serde_json::to_string_pretty(&v).expect("json") + "\n"
}

/// Compares the settings file `text` with `snapshots/<name>`; `TOKNILA_UPDATE_SNAPSHOTS=1`
/// rewrites the snapshot after an intended change.
pub(super) fn assert_settings_pinned(text: &str, name: &str) {
    let got = pinned_form(text);
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/runner/wiring/tests/snapshots");
    let path = dir.join(name);
    if std::env::var_os("TOKNILA_UPDATE_SNAPSHOTS").is_some() {
        std::fs::create_dir_all(&dir).expect("snapshot folder");
        std::fs::write(&path, &got).expect("write snapshot");
        return;
    }
    let want =
        std::fs::read_to_string(&path).expect("missing snapshot; TOKNILA_UPDATE_SNAPSHOTS=1");
    assert_eq!(got, want.replace("\r\n", "\n"), "{name}");
}

#[test]
fn a_workers_settings_file_is_pinned() {
    let rig = Rig::new();
    rig.prepare(&AgentProfile::claude_code());
    let (_, text) = rig.file("settings.json");
    assert_settings_pinned(&text, "worker-settings.json");
}

#[test]
fn a_reviewers_settings_file_is_pinned() {
    let rig = Rig::new();
    rig.prepare_as(&AgentProfile::claude_code_reviewer(), AgentRole::Reviewer);
    let (_, text) = rig.file_of("reviewer", "settings.json");
    assert_settings_pinned(&text, "reviewer-settings.json");
}
