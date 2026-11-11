//! Where a run's agent may read, judged by the allow rules alone: the deny list is emptied first,
//! so the second wall is out of the picture. Only the repository the run works in and the
//! explicit design files are allowed, for the worker, the reviewer and the planner.

use serde_json::Value;
use toknila_core::design::DesignRef;
use toknila_core::{AgentProfile, AgentRole};

use super::Rig;

/// Whether the allow rules let a `Read` (and so `Grep` and `Glob`) of `path` through: a bare tool
/// name allows everything, `Read(//dir/**)` the folder, `Read(//file)` that file.
fn allowed(allow: &[String], path: &str) -> bool {
    allow.iter().any(|r| {
        if matches!(r.as_str(), "Read" | "Grep" | "Glob") {
            return true;
        }
        let Some(rule) = r.strip_prefix("Read(/").and_then(|r| r.strip_suffix(')')) else {
            return false;
        };
        match rule.strip_suffix("/**") {
            Some(dir) => path.starts_with(&format!("{dir}/")),
            None => rule == path,
        }
    })
}

/// The allow rules of a run's settings file, read with the deny list emptied.
fn allow_without_deny(rig: &Rig, role: &str) -> Vec<String> {
    let (_, text) = rig.file_of(role, "settings.json");
    let mut v: Value = serde_json::from_str(&text).expect("json");
    v["permissions"]["deny"] = serde_json::json!([]);
    v["permissions"]["allow"]
        .as_array()
        .expect("allow")
        .iter()
        .filter_map(|r| r.as_str().map(str::to_owned))
        .collect()
}

fn check_scope(allow: &[String], repo: &str, outside: &[&str]) {
    assert!(allowed(allow, &format!("{repo}/src/lib.rs")), "{allow:?}");
    assert!(allowed(allow, &format!("{repo}/a/b/c.txt")), "{allow:?}");
    // Control: the matcher says no, to a bare tool name and to a sibling that shares a prefix.
    assert!(!allowed(allow, &format!("{repo}-other/x")), "{allow:?}");
    assert!(!allowed(&["Read".to_owned()][1..], "/x"));
    assert!(allowed(&["Read".to_owned()], "/x"));
    for p in outside {
        assert!(!allowed(allow, p), "{p} must not be readable: {allow:?}");
    }
    for bare in ["Read", "Grep", "Glob"] {
        assert!(!allow.iter().any(|r| r == bare), "{bare}: {allow:?}");
    }
}

const PRIVATE: [&str; 3] = [
    "/Users/p/.ssh/id_ed25519",
    "/Users/p/.claude/projects/x.jsonl",
    "/etc/passwd",
];

#[test]
fn the_reviewer_reads_only_the_repository_even_with_the_deny_list_emptied() {
    let rig = Rig::new();
    rig.prepare_as(&AgentProfile::claude_code_reviewer(), AgentRole::Reviewer);
    let allow = allow_without_deny(&rig, "reviewer");
    check_scope(&allow, "/w/ships/demo-1", &PRIVATE);
}

#[test]
fn the_planner_reads_only_the_repository_and_its_design_files() {
    let mut rig = Rig::new();
    rig.key = toknila_core::TicketKey::planner("DEMO".parse().expect("key"));
    rig.brief.project = Some(toknila_core::instructions::ProjectFacts {
        key: "DEMO".to_owned(),
        name: "Demo".to_owned(),
        work_branch: "main".to_owned(),
        merge_mode: toknila_core::MergeMode::Github,
        merge_method: toknila_core::MergeMethod::Squash,
        check_command: None,
        base_behind: None,
        epics: Vec::new(),
    });
    rig.brief.designs = vec!["/data/designs/p1/a.png".into()];
    rig.prepare_as(&AgentProfile::claude_code_planner(), AgentRole::Planner);
    let allow = allow_without_deny(&rig, "planner");
    check_scope(
        &allow,
        "/w/ships/demo-1",
        &[
            PRIVATE[0],
            PRIVATE[1],
            PRIVATE[2],
            "/data/designs/p1/index.json",
            "/data/designs/p1/b.png",
        ],
    );
    assert!(allowed(&allow, "/data/designs/p1/a.png"), "{allow:?}");
}

#[test]
fn the_worker_reads_only_the_repository_and_the_designs_of_its_card() {
    let mut rig = Rig::new();
    let project = toknila_core::ProjectId::new();
    let designs = crate::designs::Designs::new(rig.dir.path(), project);
    let png = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0];
    let login = designs
        .add_file("login.png", &png, toknila_core::Timestamp(1))
        .expect("kept");
    let other = designs
        .add_file("other.png", &png, toknila_core::Timestamp(2))
        .expect("kept");
    rig.brief.design_project = Some(project);
    rig.brief.card_designs = vec![DesignRef {
        design: login.id,
        region: None,
    }];
    std::fs::create_dir_all(rig.dir.path().join("runs").join("t1")).expect("run dir");
    let w = rig.prepare(&AgentProfile::claude_code());
    assert!(!w.args.is_empty());
    let allow = allow_without_deny(&rig, "worker");
    let lib = rig.dir.path().join("designs").join(project.to_string());
    let strip = |p: std::path::PathBuf| {
        toknila_core::private::read_allow_rule(&p.display().to_string())
            .trim_start_matches("Read(/")
            .trim_end_matches(')')
            .to_owned()
    };
    let mine = strip(lib.join(format!("{}.png", login.id)));
    let theirs = strip(lib.join(format!("{}.png", other.id)));
    assert!(allowed(&allow, &mine), "{allow:?}");
    check_scope(
        &allow,
        "/w/ships/demo-1",
        &[&theirs, PRIVATE[0], PRIVATE[2]],
    );
}
