//! `tok daemon set --max-checks N` through the real `tok` binary and daemon: it shows the
//! default, sets the number, and refuses 0.

mod common;

use common::Tok;

#[test]
fn max_checks_is_shown_set_and_refused_at_zero() {
    let dir = common::tempdir();
    let tok = Tok::in_dir(dir.path());
    // `tok daemon ...` does not start a daemon itself; this one is stopped by the test's guard.
    let home = dir.path().join("home");
    let started = common::start_daemon(&home);
    assert!(started.status.success(), "{started:?}");

    assert_eq!(
        tok.run(&["daemon", "set"])["max_checks"],
        1,
        "control: the default"
    );
    assert_eq!(
        tok.run(&["daemon", "set", "--max-checks", "3"])["max_checks"],
        3
    );
    assert_eq!(tok.run(&["daemon", "set"])["max_checks"], 3, "it stayed");
    assert_eq!(
        tok.text(&["daemon", "set"]).trim(),
        "checks at once: 3",
        "the words a person reads"
    );
    let err = tok.fails(&["daemon", "set", "--max-checks", "0"]);
    assert!(err.contains("at least 1"), "{err}");
    assert_eq!(
        tok.run(&["daemon", "set"])["max_checks"],
        3,
        "0 changed nothing"
    );
}
