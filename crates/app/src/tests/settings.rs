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
