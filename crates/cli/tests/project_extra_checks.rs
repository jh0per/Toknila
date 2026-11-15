//! `tok project set --extra-check` through the real `tok` binary and daemon: one flag per check,
//! the flags replace the list, `--clear-extra-checks` empties it, and a malformed check is
//! refused before anything is sent.

mod common;

use common::Tok;
use serde_json::json;

#[test]
fn extra_checks_are_set_replaced_and_cleared() {
    let dir = common::tempdir();
    let tok = Tok::in_dir(dir.path());
    tok.run(&["project", "new", "DEMO"]);
    assert!(
        tok.run(&["project", "show", "DEMO"])
            .get("extra_checks")
            .is_none(),
        "control: a new project has none"
    );

    let set = tok.run(&[
        "project",
        "set",
        "DEMO",
        "--extra-check",
        "apps/web/**=cargo xtask web && cargo xtask e2e-web",
        "--extra-check",
        "docs/**, *.md = make docs",
    ]);
    let want = json!([
        { "paths": ["apps/web/**"], "command": "cargo xtask web && cargo xtask e2e-web" },
        { "paths": ["docs/**", "*.md"], "command": "make docs" },
    ]);
    assert_eq!(set["extra_checks"], want);
    assert_eq!(tok.run(&["project", "show", "DEMO"])["extra_checks"], want);
    let text = tok.text(&[
        "project",
        "set",
        "DEMO",
        "--extra-check",
        "apps/web/**=cargo xtask web",
    ]);
    assert!(
        text.contains("extra check apps/web/**: cargo xtask web"),
        "{text}"
    );
    assert_eq!(
        tok.run(&["project", "show", "DEMO"])["extra_checks"]
            .as_array()
            .map(Vec::len),
        Some(1),
        "the flags replace the list"
    );

    let err = tok.fails(&["project", "set", "DEMO", "--extra-check", "no equals sign"]);
    assert!(err.contains("paths=command"), "{err}");

    let cleared = tok.run(&["project", "set", "DEMO", "--clear-extra-checks"]);
    assert!(cleared.get("extra_checks").is_none(), "{cleared}");
    assert!(
        tok.run(&["project", "show", "DEMO"])
            .get("extra_checks")
            .is_none()
    );
}
