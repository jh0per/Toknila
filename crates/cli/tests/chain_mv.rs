//! `tok chain mv` through the real `tok` binary and daemon: a chain goes before or after another
//! of its project, `tok chain ls` lists the roadmap in order, and a bad move is refused.

mod common;

use common::Tok;

fn order(tok: &Tok) -> Vec<String> {
    tok.run(&["chain", "ls"])
        .as_array()
        .expect("list")
        .iter()
        .map(|c| c["chain"]["name"].as_str().expect("name").to_owned())
        .collect()
}

#[test]
fn mv_reorders_the_roadmap_and_a_bad_move_is_refused() {
    let dir = common::tempdir();
    let tok = Tok::in_dir(dir.path());
    tok.run(&["project", "new", "DEMO"]);
    for n in ["one", "two", "three"] {
        tok.run(&["chain", "new", n, "--project", "DEMO"]);
    }
    assert_eq!(order(&tok), ["one", "two", "three"], "made order is kept");

    let moved = tok.run(&["chain", "mv", "three", "--before", "one"]);
    assert_eq!(moved["chain"]["name"], "three");
    assert_eq!(moved["chain"]["position"], 0);
    assert_eq!(order(&tok), ["three", "one", "two"]);

    tok.run(&["chain", "mv", "three", "--after", "two"]);
    assert_eq!(order(&tok), ["one", "two", "three"]);

    let err = tok.fails(&["chain", "mv", "one", "--after", "one"]);
    assert!(err.contains("next to itself"), "{err}");
    assert_eq!(
        order(&tok),
        ["one", "two", "three"],
        "a refused move changes nothing"
    );
    tok.fails(&["chain", "mv", "one"]);
}
