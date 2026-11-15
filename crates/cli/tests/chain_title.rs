//! `tok chain new --title` through the real `tok` binary and daemon: a title alone makes an epic
//! with the derived slug, and a title with a name keeps both.

mod common;

use common::Tok;

#[test]
fn a_title_alone_gives_the_slug_and_a_name_is_kept() {
    let dir = common::tempdir();
    let tok = Tok::in_dir(dir.path());
    tok.run(&["project", "new", "DEMO", "--sim"]);
    let made = tok.run(&[
        "chain",
        "new",
        "--title",
        "MIDI controllers",
        "--project",
        "DEMO",
        "--no-autostart",
    ]);
    assert_eq!(made["chain"]["name"], "midi-controllers", "{made}");
    assert_eq!(made["chain"]["title"], "MIDI controllers", "{made}");
    let named = tok.run(&[
        "chain",
        "new",
        "midi",
        "--title",
        "MIDI controllers",
        "--project",
        "DEMO",
        "--no-autostart",
    ]);
    assert_eq!(named["chain"]["name"], "midi", "{named}");
    assert_eq!(named["chain"]["title"], "MIDI controllers", "{named}");
}
