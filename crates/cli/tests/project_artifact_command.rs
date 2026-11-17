//! `tok project set --artifact-command` through the real `tok` binary and daemon: it sets the
//! command (trimmed, `{version}` and `{tag}` kept as typed) and an empty text clears it.

mod common;

use common::Tok;

#[test]
fn artifact_command_is_set_and_cleared() {
    let dir = common::tempdir();
    let tok = Tok::in_dir(dir.path());
    tok.run(&["project", "new", "DEMO"]);
    assert!(
        tok.run(&["project", "show", "DEMO"])
            .get("artifact_command")
            .is_none(),
        "control: a new project has none"
    );

    let set = tok.run(&[
        "project",
        "set",
        "DEMO",
        "--artifact-command",
        " ./scripts/release-mac.sh {version} {tag} ",
    ]);
    assert_eq!(
        set["artifact_command"],
        "./scripts/release-mac.sh {version} {tag}"
    );
    assert_eq!(
        tok.run(&["project", "show", "DEMO"])["artifact_command"],
        "./scripts/release-mac.sh {version} {tag}"
    );

    let cleared = tok.run(&["project", "set", "DEMO", "--artifact-command", ""]);
    assert!(cleared.get("artifact_command").is_none(), "{cleared}");
    assert!(
        tok.run(&["project", "show", "DEMO"])
            .get("artifact_command")
            .is_none()
    );
}
