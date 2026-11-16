//! `tok release steps <epic>`, `tok release mark <epic> --with <tag>` and the one planned version
//! per project through the real `tok` binary and daemon.

mod common;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use common::Tok;

/// A local-mode project with an epic `phase` planned at `version` and one merged story.
fn finished_epic(tok: &Tok, dir: &Path, version: &str) -> PathBuf {
    let repo = common::repo(dir);
    tok.run(&[
        "project",
        "new",
        "DEMO",
        "--sim",
        "--repo",
        &repo.to_string_lossy(),
        "--base",
        "develop",
        "--merge",
        "local",
    ]);
    tok.run(&["review-mode", "human", "--project", "DEMO"]);
    let chain = dir.join("chain.toml");
    std::fs::write(
        &chain,
        format!(
            "[chain]\nname = \"phase\"\nautostart = true\nversion = \"{version}\"\n\n[[ticket]]\nid = \"work\"\ntitle = \"work\"\nprofile = \"sim\"\n"
        ),
    )
    .expect("chain file");
    tok.run(&[
        "chain",
        "import",
        &chain.to_string_lossy(),
        "--project",
        "DEMO",
    ]);
    let until = Instant::now() + Duration::from_secs(30);
    let ship = loop {
        if let Some(p) = tok.run(&["show", "DEMO-1"])["ship"]["path"].as_str() {
            break PathBuf::from(p);
        }
        assert!(Instant::now() < until, "DEMO-1 has no workspace folder");
        std::thread::sleep(Duration::from_millis(100));
    };
    std::fs::write(ship.join("done.txt"), "done\n").expect("write");
    common::git(&ship, &["add", "done.txt"]);
    common::git(&ship, &["commit", "--quiet", "-m", "done"]);
    tok.run(&["sim", "DEMO-1", "finish", "all done"]);
    tok.wait_for_person_turn("DEMO-1");
    tok.run(&["accept", "DEMO-1"]);
    let until = Instant::now() + Duration::from_secs(30);
    while tok.status("DEMO-1") != "merged" {
        assert!(Instant::now() < until, "DEMO-1 did not merge");
        std::thread::sleep(Duration::from_millis(100));
    }
    repo
}

#[test]
fn release_steps_and_run_answer_to_the_epics_name() {
    let dir = common::tempdir();
    let tok = Tok::in_dir(dir.path());
    let repo = finished_epic(&tok, dir.path(), "0.2.0");

    // The version is left out: the epic's own.
    tok.run(&["release", "run", "phase", "--no-push"]);
    let until = Instant::now() + Duration::from_secs(30);
    let by_name = loop {
        let text = tok.text(&["release", "steps", "phase"]);
        if text.contains("released") {
            break text;
        }
        assert!(Instant::now() < until, "the run did not finish: {text}");
        std::thread::sleep(Duration::from_millis(100));
    };
    assert!(
        by_name.starts_with("Release 0.2.0 (v0.2.0): released"),
        "{by_name}"
    );
    assert_eq!(by_name, tok.text(&["release", "steps", "0.2.0"]));
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(&repo)
        .args(["tag", "--list"])
        .output()
        .expect("git");
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "v0.2.0");
}

#[test]
fn a_person_marks_an_epic_released_with_an_old_tag_and_a_wrong_tag_is_refused() {
    let dir = common::tempdir();
    let tok = Tok::in_dir(dir.path());
    let repo = finished_epic(&tok, dir.path(), "0.3.4");

    let said = tok.fails(&["release", "mark", "phase", "--with", "v9.9.9"]);
    assert!(
        said.contains("the tag v9.9.9 does not exist in the repository"),
        "{said}"
    );
    common::git(&repo, &["tag", "v0.3.3"]);
    tok.run(&["release", "mark", "phase", "--with", "v0.3.3"]);
    assert_eq!(tok.status("DEMO-1"), "released");
    let shown = tok.text(&["chain", "show", "phase"]);
    assert!(
        shown.contains("Release v0.3.3: released, tag v0.3.3"),
        "{shown}"
    );
    assert!(shown.contains("released with v0.3.3"), "{shown}");
}

#[test]
fn a_version_another_epic_plans_is_refused_with_the_epic_named() {
    let dir = common::tempdir();
    let tok = Tok::in_dir(dir.path());
    tok.run(&["project", "new", "DEMO", "--sim"]);
    tok.run(&[
        "chain",
        "new",
        "phase-10",
        "--project",
        "DEMO",
        "--version",
        "0.3.4",
    ]);
    let said = tok.fails(&[
        "chain",
        "new",
        "phase-11",
        "--project",
        "DEMO",
        "--version",
        "0.3.4",
    ]);
    assert!(
        said.contains("0.3.4 is planned by phase-10; pick another version or move that one"),
        "{said}"
    );
    tok.run(&[
        "chain",
        "new",
        "phase-11",
        "--project",
        "DEMO",
        "--version",
        "0.3.5",
    ]);
    let said = tok.fails(&[
        "chain",
        "set",
        "phase-11",
        "--project",
        "DEMO",
        "--version",
        "0.3.4",
    ]);
    assert!(said.contains("0.3.4 is planned by phase-10"), "{said}");
}
