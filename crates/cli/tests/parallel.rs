//! Parallel agents through the real `tok` binary and daemon: the limit defaults to two, a card
//! started beyond it waits in a queue that `tok ls` shows, and `tok project set --parallel`
//! changes the limit.

mod common;

use common::Tok;

const CHAIN: &str = r#"
[chain]
name = "demo"
goal = "Login and profile side by side"
autostart = false

[[ticket]]
id = "DESIGN"
title = "Design"
profile = "sim"

[[ticket]]
id = "LOGIN"
title = "Login"
after = ["DESIGN"]
profile = "sim"

[[ticket]]
id = "PROFILE"
title = "Profile page"
after = ["DESIGN"]
profile = "sim"

[[ticket]]
id = "SETTINGS"
title = "Settings"
after = ["DESIGN"]
profile = "sim"
"#;

fn queued_note(tok: &Tok, key: &str) -> Option<u64> {
    tok.run(&["show", key])["queue_position"].as_u64()
}

#[test]
fn a_card_started_beyond_the_limit_waits_and_the_limit_is_settable() {
    let dir = common::tempdir();
    let tok = Tok::in_dir(dir.path());
    let file = dir.path().join("demo.toml");
    std::fs::write(&file, CHAIN).expect("write chain");
    tok.run(&["project", "new", "DEMO", "--name", "Demo"]);
    tok.run(&[
        "chain",
        "import",
        file.to_str().expect("utf-8 path"),
        "--project",
        "DEMO",
    ]);

    // The design card goes first and alone; once merged the three others are Ready.
    tok.run(&["start", "DEMO-1"]);
    tok.run(&["sim", "DEMO-1", "finish", "designed"]);
    tok.run(&["accept", "DEMO-1"]);
    assert_eq!(
        tok.statuses(),
        [
            "DEMO-1 merged",
            "DEMO-2 ready",
            "DEMO-3 ready",
            "DEMO-4 ready"
        ]
    );

    for key in ["DEMO-2", "DEMO-3", "DEMO-4"] {
        tok.run(&["start", key]);
    }
    assert_eq!(
        tok.statuses(),
        [
            "DEMO-1 merged",
            "DEMO-2 running",
            "DEMO-3 running",
            "DEMO-4 ready"
        ]
    );
    assert_eq!(queued_note(&tok, "DEMO-4"), Some(1));
    assert_eq!(queued_note(&tok, "DEMO-2"), None);

    // The plain output says so too.
    let plain = std::process::Command::new(env!("CARGO_BIN_EXE_tok"))
        .args(["ls"])
        .env("TOKNILA_HOME", &tok.home)
        .output()
        .expect("run tok ls");
    let text = String::from_utf8_lossy(&plain.stdout);
    assert!(text.contains("ready (queued #1)"), "{text}");

    // More room starts the queue at once; less never stops anyone.
    let p = tok.run(&["project", "set", "DEMO", "--parallel", "3"]);
    assert_eq!(p["max_parallel"], 3);
    assert_eq!(tok.status("DEMO-4"), "running");
    tok.run(&["project", "set", "DEMO", "--parallel", "1"]);
    assert_eq!(
        (
            tok.status("DEMO-2"),
            tok.status("DEMO-3"),
            tok.status("DEMO-4")
        ),
        (
            "running".to_owned(),
            "running".to_owned(),
            "running".to_owned()
        )
    );

    let err = tok.fails(&["project", "set", "DEMO", "--parallel", "0"]);
    assert!(err.contains("between 1 and 64"), "{err}");
}
