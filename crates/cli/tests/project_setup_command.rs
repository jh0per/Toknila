//! `tok project set --setup-command` through the real `tok` binary and daemon: it sets the
//! command (trimmed) and an empty text clears it.

mod common;

use common::Tok;

#[test]
fn setup_command_is_set_and_cleared() {
    let dir = common::tempdir();
    let tok = Tok::in_dir(dir.path());
    tok.run(&["project", "new", "DEMO"]);
    assert!(
        tok.run(&["project", "show", "DEMO"])
            .get("setup_command")
            .is_none(),
        "control: a new project has none"
    );

    let set = tok.run(&[
        "project",
        "set",
        "DEMO",
        "--setup-command",
        " npm ci --prefix apps/web ",
    ]);
    assert_eq!(set["setup_command"], "npm ci --prefix apps/web");
    assert_eq!(
        tok.run(&["project", "show", "DEMO"])["setup_command"],
        "npm ci --prefix apps/web"
    );

    let cleared = tok.run(&["project", "set", "DEMO", "--setup-command", ""]);
    assert!(cleared.get("setup_command").is_none(), "{cleared}");
    assert!(
        tok.run(&["project", "show", "DEMO"])
            .get("setup_command")
            .is_none()
    );
}
