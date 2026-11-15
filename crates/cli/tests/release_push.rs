//! `tok release run --push` / `--no-push` through the real `tok` binary and daemon: asking for the
//! push in a repository without a remote is refused with a sentence and tags nothing, the two
//! flags exclude each other, and `--no-push` keeps the tag local.

mod common;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use common::Tok;

/// A local-mode project with one merged story, so the epic can be released.
fn finished_epic(tok: &Tok, dir: &Path) -> PathBuf {
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
        "[chain]\nname = \"phase\"\nautostart = true\n\n[[ticket]]\nid = \"work\"\ntitle = \"work\"\nprofile = \"sim\"\n",
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

fn tags(repo: &Path) -> String {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["tag", "--list"])
        .output()
        .expect("git");
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

#[test]
fn push_without_a_remote_is_refused_with_a_sentence_and_no_push_tags_locally() {
    let dir = common::tempdir();
    let tok = Tok::in_dir(dir.path());
    let repo = finished_epic(&tok, dir.path());

    let said = tok.fails(&["release", "run", "phase", "0.1.0", "--push"]);
    assert!(said.contains("no remote to push to"), "{said}");
    assert_eq!(tags(&repo), "", "nothing was tagged");

    let said = tok.fails(&["release", "run", "phase", "0.1.0", "--push", "--no-push"]);
    assert!(said.contains("cannot be used with"), "{said}");

    tok.run(&["release", "run", "phase", "0.1.0", "--no-push"]);
    let until = Instant::now() + Duration::from_secs(30);
    while tags(&repo) != "v0.1.0" {
        assert!(Instant::now() < until, "the tag was not made");
        std::thread::sleep(Duration::from_millis(100));
    }
    let steps = tok.text(&["release", "steps", "0.1.0"]);
    assert!(steps.contains("Tag") && !steps.contains("Push"), "{steps}");
}

fn develop_head(repo: &Path) -> String {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["rev-parse", "refs/heads/develop"])
        .output()
        .expect("git");
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

#[test]
fn push_with_a_remote_lands_the_base_and_the_tag_there() {
    let dir = common::tempdir();
    let tok = Tok::in_dir(dir.path());
    let repo = finished_epic(&tok, dir.path());
    let remote = dir.path().join("remote.git");
    std::fs::create_dir_all(&remote).expect("mkdir");
    common::git(&remote, &["init", "--quiet", "--bare"]);
    common::git(
        &repo,
        &["remote", "add", "origin", &remote.to_string_lossy()],
    );

    tok.run(&["release", "run", "phase", "0.1.0", "--push"]);
    let until = Instant::now() + Duration::from_secs(30);
    while tags(&remote) != "v0.1.0" {
        assert!(
            Instant::now() < until,
            "the tag did not reach the remote: {}",
            tok.text(&["release", "steps", "0.1.0"])
        );
        std::thread::sleep(Duration::from_millis(100));
    }
    let steps = tok.text(&["release", "steps", "0.1.0"]);
    assert!(
        steps.contains("Pushed develop and v0.1.0 to origin"),
        "{steps}"
    );
    assert_eq!(
        develop_head(&repo),
        develop_head(&remote),
        "develop reached the remote"
    );
}
