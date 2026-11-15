//! `tok ship ls` through the real `tok` binary and daemon: a started story's workspace is listed
//! with its status, and a cancelled story leaves none.

mod common;

use common::{Tok, repo};

#[test]
fn ls_lists_a_story_workspace_with_its_status_and_none_once_it_is_cancelled() {
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
    let mut listed = serde_json::Value::Null;
    for _ in 0..100 {
        listed = tok.run(&["ship", "ls"]);
        if listed["count"] == 1 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    assert_eq!(listed["count"], 1, "{listed}");
    assert_eq!(listed["finished"], 0);
    let row = &listed["items"][0];
    assert_eq!(
        (row["story"].as_str(), row["kind"].as_str()),
        (Some("DEMO-1"), Some("story"))
    );
    assert_eq!(row["status"], "running");
    assert_eq!(row["finished"], false);

    tok.run(&["cancel", "DEMO-1", "not needed"]);
    // The retirement runs right after the cancel commits: until it has, the row shows a finished
    // story; once it has, there is none.
    let mut after = serde_json::Value::Null;
    for _ in 0..100 {
        after = tok.run(&["ship", "ls"]);
        if after["count"] == 0 {
            break;
        }
        assert_eq!(after["items"][0]["status"], "cancelled", "{after}");
        assert_eq!(after["items"][0]["finished"], true, "{after}");
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    assert_eq!(
        (after["count"].as_u64(), after["finished"].as_u64()),
        (Some(0), Some(0)),
        "{after}"
    );
}
