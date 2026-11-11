//! `tok ship prune` through the real `tok` binary and daemon: a cancelled card's branch that kept
//! its work is listed with the card's key, and pruning deletes it.

mod common;

use common::{Tok, git, repo};

fn branches(repo: &std::path::Path) -> String {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["branch", "--list", "ship/*"])
        .output()
        .expect("git runs");
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

#[test]
fn prune_lists_and_deletes_the_branch_a_finished_card_left() {
    let dir = tempfile::tempdir().expect("tempdir");
    let tok = Tok::in_dir(dir.path());
    let repo = repo(dir.path());
    let path = repo.to_str().expect("utf-8 path");
    tok.run(&["project", "new", "DEMO", "--repo", path]);
    tok.run(&[
        "chain",
        "new",
        "auth",
        "--project",
        "DEMO",
        "--no-autostart",
    ]);
    tok.run(&["ticket", "new", "auth", "Login"]);
    tok.run(&["start", "DEMO-1"]);
    // The worktree is made right after the start commits.
    let mut ship = None;
    for _ in 0..100 {
        let shown = tok.run(&["show", "DEMO-1"]);
        if let Some(p) = shown["ship"]["path"].as_str() {
            ship = Some(std::path::PathBuf::from(p));
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    let ship = ship.expect("the card gets a ship");
    std::fs::write(ship.join("login.txt"), "login\n").expect("write");
    git(&ship, &["add", "login.txt"]);
    git(&ship, &["commit", "--quiet", "-m", "add login"]);
    tok.run(&["cancel", "DEMO-1", "not needed"]);
    assert!(branches(&repo).contains("demo-1-login"), "the work is kept");

    let listed = tok.run(&["ship", "prune", "--dry-run"]);
    assert_eq!(listed["items"][0]["card"], "DEMO-1", "{listed}");
    assert_eq!(listed["items"][0]["kind"], "branch");
    assert!(
        branches(&repo).contains("demo-1-login"),
        "dry run deletes nothing"
    );

    let done = tok.run(&["ship", "prune"]);
    assert_eq!(done["removed"][0]["card"], "DEMO-1", "{done}");
    assert_eq!(done["warnings"], serde_json::json!([]));
    assert_eq!(branches(&repo), "");
    assert_eq!(
        tok.run(&["ship", "prune", "--dry-run"])["items"],
        serde_json::json!([])
    );
}
