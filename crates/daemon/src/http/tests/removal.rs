//! Deleting a project through the router, on a temporary git repository: the records, worktrees
//! and run files go, the repository folder and its `ship/*` branch stay, or the folder goes to
//! the (fake) Trash when asked. Refused while a card runs.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};

use super::Api;
use super::ships::{project, repo};
use crate::git::testing::{commit_file, git};
use crate::plan_files::canonical;
use crate::shipyard::GitShipyard;

/// A daemon with a shipyard, project DEMO on a fresh repository (chain `auth`, autostart), and
/// card DEMO-1 handed in: in Review, its worktree kept, no agent working.
async fn reviewed(tmp: &Path) -> (Api, PathBuf, PathBuf) {
    let repo = repo(tmp);
    let api = Api::with_yard(Some(Arc::new(GitShipyard::new(
        tmp.join("home").join("ships"),
    ))));
    project(&api, &repo).await;
    api.post("/v1/chains/auth/tickets", json!({ "title": "Login" }))
        .await;
    let d = api.get("/v1/tickets/DEMO-1").await;
    let ship = PathBuf::from(d["ship"]["path"].as_str().expect("a ship"));
    commit_file(&ship, "login.txt", "login\n", "add login");
    api.post(
        "/v1/tickets/DEMO-1/events",
        json!({ "event": "finished", "summary": "done" }),
    )
    .await;
    assert_eq!(api.status("DEMO-1").await, "review");
    (api, repo, ship)
}

async fn delete(api: &Api, key: &str, trash_folder: bool) -> (StatusCode, Value) {
    api.call(
        Method::POST,
        &format!("/v1/projects/{key}/delete"),
        Some(json!({ "trash_folder": trash_folder })),
    )
    .await
}

async fn gone(api: &Api, uri: &str) -> bool {
    api.call(Method::GET, uri, None).await.0 == StatusCode::NOT_FOUND
}

#[tokio::test]
async fn records_only_removes_the_rows_worktrees_and_run_files_and_keeps_the_folder_and_branch() {
    let tmp = tempfile::tempdir().expect("tmp");
    let (api, repo, ship) = reviewed(tmp.path()).await;
    api.post("/v1/projects", json!({ "key": "KEEP" })).await;
    api.post("/v1/projects/KEEP/chains", json!({ "name": "kept" }))
        .await;
    api.post("/v1/chains/kept/tickets", json!({ "title": "stays" }))
        .await;
    let ticket = api.get("/v1/tickets/DEMO-1").await["ticket"]["id"]
        .as_str()
        .expect("id")
        .to_owned();
    let runs = api._dir.path().join("runs").join(&ticket);
    std::fs::create_dir_all(&runs).expect("mkdir");
    std::fs::write(runs.join("run.log"), "output\n").expect("log");

    let (s, out) = delete(&api, "DEMO", false).await;
    assert_eq!(s, StatusCode::OK, "{out}");
    assert_eq!(out["project"]["key"], "DEMO");
    assert_eq!(out["chains"], json!(["auth"]));
    assert_eq!(out["tickets"], 1);
    assert_eq!(out["worktrees_removed"], json!([ship.to_string_lossy()]));
    assert_eq!(out["folder"], "kept");
    assert_eq!(out["warnings"], json!([]));
    assert!(out.get("folder_error").is_none(), "{out}");

    for uri in ["/v1/projects/DEMO", "/v1/chains/auth", "/v1/tickets/DEMO-1"] {
        assert!(gone(&api, uri).await, "{uri}");
    }
    assert!(!ship.exists(), "the worktree is removed");
    assert!(!runs.exists(), "the run files are removed");
    assert!(repo.join("README.md").is_file(), "the repository stays");
    assert_eq!(
        git(&repo, &["branch", "--list", "ship/auth/demo-1-login"]),
        "ship/auth/demo-1-login"
    );
    assert_eq!(git(&repo, &["worktree", "list"]).lines().count(), 1);
    assert_eq!(api.status("KEEP-1").await, "running");

    let events = api.get("/v1/events?since=0").await;
    let events = events.as_array().expect("events");
    assert!(
        events.iter().all(|e| e["entity_id"] != ticket.as_str()),
        "the card's journal entries went with it"
    );
    let tail: Vec<String> = events
        .iter()
        .rev()
        .take(2)
        .map(|e| {
            format!(
                "{}.{}",
                e["entity"].as_str().expect("entity"),
                e["kind"].as_str().expect("kind")
            )
        })
        .collect();
    assert_eq!(tail, ["project.deleted", "chain.deleted"]);
    assert_eq!(
        events.iter().filter(|e| e["entity"] == "project").count(),
        2,
        "KEEP created, DEMO deleted"
    );
}

#[tokio::test]
async fn with_trash_folder_the_repository_goes_to_the_trash() {
    let tmp = tempfile::tempdir().expect("tmp");
    let (api, repo, ship) = reviewed(tmp.path()).await;
    let resolved = canonical(&repo);
    let (s, out) = delete(&api, "DEMO", true).await;
    assert_eq!(s, StatusCode::OK, "{out}");
    assert_eq!(out["folder"], "trashed", "{out}");
    assert!(!repo.exists());
    assert!(!ship.exists(), "the worktree went first");
    let bin = api._dir.path().join("trash").join("repo");
    assert!(bin.join("README.md").is_file(), "the fake Trash holds it");
    assert_eq!(
        *api.trash.moved.lock().expect("lock"),
        [resolved],
        "exactly the repository folder"
    );
}

#[tokio::test]
async fn a_folder_inside_the_data_home_is_never_trashed_and_the_records_still_go() {
    let api = Api::new();
    let inside = repo(api._dir.path());
    project(&api, &inside).await;
    let (s, out) = delete(&api, "DEMO", true).await;
    assert_eq!(s, StatusCode::OK, "{out}");
    assert_eq!(out["folder"], "kept");
    let error = out["folder_error"].as_str().expect("why");
    assert!(error.contains("Toknila's own data"), "{error}");
    assert!(inside.join("README.md").is_file());
    assert!(api.trash.moved.lock().expect("lock").is_empty());
    assert!(gone(&api, "/v1/projects/DEMO").await);
}

#[tokio::test]
async fn a_project_without_a_folder_has_none_to_trash() {
    let api = Api::new();
    api.demo().await;
    let (s, out) = delete(&api, "DEMO", true).await;
    assert_eq!(s, StatusCode::OK, "{out}");
    assert_eq!(
        (out["folder"].as_str(), out.get("folder_error")),
        (Some("none"), None)
    );
    assert!(gone(&api, "/v1/chains/demo").await);
}

#[tokio::test]
async fn deleting_is_refused_while_a_card_runs() {
    let api = Api::new();
    api.demo().await;
    api.post("/v1/chains/demo/tickets", json!({ "title": "A" }))
        .await;
    assert_eq!(api.status("DEMO-1").await, "running");
    let (s, e) = delete(&api, "DEMO", false).await;
    assert_eq!(s, StatusCode::CONFLICT, "{e}");
    assert_eq!(
        e["error"],
        "DEMO has 1 agent(s) working; stop them before deleting the project"
    );
    assert_eq!(api.status("DEMO-1").await, "running");
    assert_eq!(api.get("/v1/projects/DEMO").await["key"], "DEMO");
}
