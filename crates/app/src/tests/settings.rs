//! Project settings the window edits: default profile and base branch next to the limit and the
//! review settings.

use super::Mem;
use crate::ErrorKind;
use crate::projects::{self, PatchProject};

#[test]
fn the_default_profile_and_base_branch_can_be_changed() {
    let mut mem = Mem::demo();
    let patch = PatchProject {
        default_profile: Some("claude-code".to_owned()),
        work_branch: Some(" trunk ".to_owned()),
        ..PatchProject::default()
    };
    let p = projects::patch(&mut mem, "DEMO", patch).expect("patch");
    assert_eq!(p.default_profile, "claude-code");
    assert_eq!(p.work_branch, "trunk");
    assert_eq!(mem.project.work_branch, "trunk");
}

#[test]
fn a_project_created_without_a_profile_runs_the_real_agent_and_sim_is_a_default_nowhere() {
    let mut mem = Mem::demo();
    let p = projects::create(
        &mut mem,
        projects::NewProject {
            key: "REAL".to_owned(),
            ..projects::NewProject::default()
        },
    )
    .expect("create");
    assert_eq!(p.default_profile, "claude-code");
    assert!(!p.simulated);
}

#[test]
fn choosing_sim_by_name_marks_the_project_simulated() {
    let mut mem = Mem::demo();
    mem.project.simulated = false;
    let patch = PatchProject {
        default_profile: Some("sim".to_owned()),
        ..PatchProject::default()
    };
    let p = projects::patch(&mut mem, "DEMO", patch).expect("patch");
    assert!(p.simulated);
}

#[test]
fn a_blank_profile_or_branch_is_refused_and_changes_nothing() {
    let mut mem = Mem::demo();
    let before = mem.project.clone();
    for patch in [
        PatchProject {
            default_profile: Some("  ".to_owned()),
            ..PatchProject::default()
        },
        PatchProject {
            work_branch: Some(String::new()),
            ..PatchProject::default()
        },
    ] {
        let err = projects::patch(&mut mem, "DEMO", patch).expect_err("refused");
        assert_eq!(err.kind, ErrorKind::BadRequest);
    }
    assert_eq!(mem.project, before);
}

#[test]
fn the_check_command_is_set_and_cleared_through_the_patch() {
    let mut mem = Mem::demo();
    let set = PatchProject {
        check_command: Some("cargo xtask verify".to_owned()),
        ..PatchProject::default()
    };
    let p = projects::patch(&mut mem, "DEMO", set).expect("patch");
    assert_eq!(p.check_command.as_deref(), Some("cargo xtask verify"));
    let other = PatchProject {
        max_parallel: Some(3),
        ..PatchProject::default()
    };
    let p = projects::patch(&mut mem, "DEMO", other).expect("patch");
    assert_eq!(
        p.check_command.as_deref(),
        Some("cargo xtask verify"),
        "unset stays"
    );
    let clear = PatchProject {
        check_command: Some(String::new()),
        ..PatchProject::default()
    };
    assert_eq!(
        projects::patch(&mut mem, "DEMO", clear)
            .expect("patch")
            .check_command,
        None
    );
    let bad = PatchProject {
        check_command: Some("a\nb".to_owned()),
        ..PatchProject::default()
    };
    assert_eq!(
        projects::patch(&mut mem, "DEMO", bad)
            .expect_err("bad")
            .kind,
        ErrorKind::BadRequest
    );
}
