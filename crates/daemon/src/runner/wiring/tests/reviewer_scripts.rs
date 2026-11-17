//! The reviewer's settings and instructions carry the exact commands of the hand-in's scripts.

use std::process::Command;

use serde_json::Value;
use toknila_core::{AgentProfile, AgentRole};

use super::Rig;

fn git(repo: &std::path::Path, args: &[&str]) {
    let status = Command::new("git")
        .args(args)
        .current_dir(repo)
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?}");
}

#[test]
fn the_reviewer_may_run_a_tracked_script_the_hand_in_names_and_nothing_wider() {
    let repo = tempfile::tempdir().expect("tempdir");
    git(repo.path(), &["init", "-q"]);
    std::fs::create_dir_all(repo.path().join("scripts")).expect("mkdir");
    for f in ["scripts/test_it.py", "scripts/untracked.py"] {
        std::fs::write(repo.path().join(f), "print(1)\n").expect("write");
    }
    git(repo.path(), &["add", "scripts/test_it.py"]);
    let mut rig = Rig::new();
    rig.repo = repo.path().to_path_buf();
    rig.brief.hand_in = Some(
        "**What changed:** x\n**Checks:**\n\
         - PASS python3 scripts/test_it.py: 4 passed\n\
         - PASS python3 scripts/untracked.py: 4 passed\n\
         - PASS python3 scripts/missing.py: 4 passed\n\
         - PASS python3 -c print(1): 1\n\
         - PASS python3 scripts/test_it.py && ls: 4 passed\n\
         **Left:** nothing"
            .to_owned(),
    );
    rig.prepare_as(&AgentProfile::claude_code_reviewer(), AgentRole::Reviewer);
    let (_, text) = rig.file_of("reviewer", "settings.json");
    let v: Value = serde_json::from_str(&text).expect("json");
    let allow: Vec<&str> = v["permissions"]["allow"]
        .as_array()
        .expect("rules")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    let python: Vec<&&str> = allow.iter().filter(|r| r.contains("python3")).collect();
    assert_eq!(
        python,
        [&"Bash(python3 -B scripts/test_it.py)"],
        "{allow:?}"
    );
    let (_, instructions) = rig.file_of("reviewer", "instructions.md");
    assert!(
        instructions.contains("- `python3 -B scripts/test_it.py`"),
        "{instructions}"
    );
    assert!(!instructions.contains("untracked.py`"), "{instructions}");
}
