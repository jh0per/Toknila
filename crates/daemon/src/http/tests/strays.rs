//! Cancelling a card and the leftovers list, through the router on a temporary git repository.

use std::path::Path;
use std::sync::Arc;

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};

use super::Api;
use super::ships::{project, repo};
use crate::git::testing::{commit_file, git};
use crate::shipyard::GitShipyard;

const BRANCH: &str = "ship/auth/demo-1-login";

async fn api_with_card(tmp: &Path) -> (Api, std::path::PathBuf, std::path::PathBuf) {
    let repo = repo(tmp);
    let api = Api::with_yard(Some(Arc::new(GitShipyard::new(
        tmp.join("home").join("ships"),
    ))));
    project(&api, &repo).await;
    api.post("/v1/chains/auth/tickets", json!({ "title": "Login" }))
        .await;
    let d = api.get("/v1/tickets/DEMO-1").await;
    let ship = std::path::PathBuf::from(d["ship"]["path"].as_str().expect("a ship"));
    (api, repo, ship)
}

async fn cancel(api: &Api, key: &str) {
    api.post(
        &format!("/v1/tickets/{key}/actions/cancel"),
        json!({ "text": "not needed" }),
    )
    .await;
}

fn branches(repo: &Path) -> String {
    git(repo, &["branch", "--list", "ship/*"])
}

#[tokio::test]
async fn cancelling_a_card_without_work_leaves_no_worktree_and_no_branch() {
    let tmp = tempfile::tempdir().expect("tmp");
    let (api, repo, ship) = api_with_card(tmp.path()).await;
    assert!(ship.exists());
    assert!(branches(&repo).contains(BRANCH));
    cancel(&api, "DEMO-1").await;
    let d = api.get("/v1/tickets/DEMO-1").await;
    assert_eq!(d["ticket"]["status"], "cancelled");
    assert_eq!(d["ship"], Value::Null);
    assert!(!ship.exists());
    assert_eq!(branches(&repo), "", "the empty branch is deleted");
    assert!(d["comments"].as_array().expect("comments").is_empty());
    let strays = api.get("/v1/projects/DEMO/strays").await;
    assert_eq!(strays["items"], json!([]));
}

#[tokio::test]
async fn cancelling_a_card_with_commits_keeps_the_branch_and_names_it_on_the_card() {
    let tmp = tempfile::tempdir().expect("tmp");
    let (api, repo, ship) = api_with_card(tmp.path()).await;
    commit_file(&ship, "login.txt", "login\n", "add login");
    cancel(&api, "DEMO-1").await;
    let d = api.get("/v1/tickets/DEMO-1").await;
    assert_eq!(d["ticket"]["status"], "cancelled");
    assert!(!ship.exists(), "the worktree is removed");
    assert!(branches(&repo).contains(BRANCH), "the branch has work");
    let comments = d["comments"].as_array().expect("comments");
    assert_eq!(comments.len(), 1, "{comments:?}");
    let gist = comments[0]["body"]["gist"].as_str().expect("gist");
    assert!(gist.contains(BRANCH) && gist.contains("1 commit"), "{gist}");
}

#[tokio::test]
async fn the_list_shows_what_no_live_card_owns_and_clean_up_removes_it() {
    let tmp = tempfile::tempdir().expect("tmp");
    let (api, repo, ship) = api_with_card(tmp.path()).await;
    // A second chain whose card stays live, and a first card that is cancelled with work.
    api.post("/v1/projects/DEMO/chains", json!({ "name": "other" }))
        .await;
    api.post("/v1/chains/other/tickets", json!({ "title": "Live" }))
        .await;
    let live = api.get("/v1/tickets/DEMO-2").await;
    let live_path = live["ship"]["path"].as_str().expect("live ship").to_owned();
    commit_file(&ship, "login.txt", "login\n", "add login");
    cancel(&api, "DEMO-1").await;
    // A worktree nobody owns: made by hand next to the live ones, as a crash would leave it.
    let orphan = ship.parent().expect("chain folder").join("orphan");
    git(
        &repo,
        &[
            "worktree",
            "add",
            "-b",
            "ship/auth/orphan",
            &orphan.to_string_lossy(),
            "develop",
        ],
    );

    let strays = api.get("/v1/projects/DEMO/strays").await;
    let items = strays["items"].as_array().expect("items");
    let name = |i: &Value| i["name"].as_str().expect("name").to_owned();
    let trees: Vec<_> = items
        .iter()
        .filter(|i| i["kind"] == "worktree")
        .map(name)
        .collect();
    let branches_found: Vec<_> = items
        .iter()
        .filter(|i| i["kind"] == "branch")
        .map(|i| (name(i), i["ahead"].clone()))
        .collect();
    assert_eq!(trees.len(), 1, "{items:?}");
    assert!(trees[0].ends_with("orphan"), "{trees:?}");
    assert_eq!(
        branches_found,
        [
            (BRANCH.to_owned(), json!(1)),
            ("ship/auth/orphan".to_owned(), json!(0))
        ],
        "the live card's branch is not listed"
    );

    let (status, done) = api
        .call(Method::POST, "/v1/projects/DEMO/strays/clean", None)
        .await;
    assert_eq!(status, StatusCode::OK, "{done}");
    assert_eq!(done["removed"].as_array().expect("removed").len(), 3);
    assert_eq!(done["warnings"], json!([]));
    assert!(!orphan.exists());
    let left = branches(&repo);
    assert!(!left.contains(BRANCH) && !left.contains("orphan"), "{left}");
    assert!(
        left.contains("demo-2-live"),
        "the live card's branch stays: {left}"
    );
    assert!(Path::new(&live_path).exists(), "and so does its worktree");
    let again = api.get("/v1/projects/DEMO/strays").await;
    assert_eq!(again["items"], json!([]));
}

#[tokio::test]
async fn a_deleted_chain_leaves_its_branch_in_the_list() {
    let tmp = tempfile::tempdir().expect("tmp");
    let (api, repo, ship) = api_with_card(tmp.path()).await;
    commit_file(&ship, "login.txt", "login\n", "add login");
    api.post(
        "/v1/tickets/DEMO-1/events",
        json!({ "event": "finished", "summary": toknila_core::handin::from_plain("done") }),
    )
    .await;
    let (status, _) = api.call(Method::DELETE, "/v1/chains/auth", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(branches(&repo).contains(BRANCH), "deleting keeps branches");
    let strays = api.get("/v1/projects/DEMO/strays").await;
    assert_eq!(strays["items"][0]["name"], BRANCH);
    assert_eq!(strays["items"][0]["kind"], "branch");
}
