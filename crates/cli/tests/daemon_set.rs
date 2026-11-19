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
        "checks at once: 3\nsound, a story is done: off\nsound, an epic is finished: off\n\
         sound, something went wrong: off",
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

#[test]
fn the_sounds_are_off_set_one_by_one_and_kept_when_the_number_changes() {
    let dir = common::tempdir();
    let tok = Tok::in_dir(dir.path());
    let home = dir.path().join("home");
    let started = common::start_daemon(&home);
    assert!(started.status.success(), "{started:?}");

    let off = serde_json::json!({ "story_done": false, "epic_done": false, "problem": false });
    assert_eq!(
        tok.run(&["daemon", "set"])["sounds"],
        off,
        "control: all off"
    );
    let one = tok.run(&["daemon", "set", "--sound-epic-done", "on"]);
    assert_eq!(
        one["sounds"],
        serde_json::json!({ "story_done": false, "epic_done": true, "problem": false })
    );
    assert_eq!(one["max_checks"], 1, "the number stayed");
    let two = tok.run(&[
        "daemon",
        "set",
        "--sound-problem",
        "on",
        "--max-checks",
        "2",
    ]);
    assert_eq!(
        two["sounds"],
        serde_json::json!({ "story_done": false, "epic_done": true, "problem": true })
    );
    assert_eq!(two["max_checks"], 2);
    let text = tok.text(&["daemon", "set", "--sound-epic-done", "off"]);
    assert!(text.contains("sound, an epic is finished: off"), "{text}");
    assert!(text.contains("sound, something went wrong: on"), "{text}");
    let err = tok.fails(&["daemon", "set", "--sound-problem", "loud"]);
    assert!(err.contains("on") && err.contains("off"), "{err}");
}
