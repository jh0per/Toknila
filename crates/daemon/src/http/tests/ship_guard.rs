//! The uncommitted-ship guard through the full router, with the real git shipyard and merger on
//! temporary repositories: `finished` is refused (and the card stays Running) while the ship has
//! changes that are not committed or no commit of its own; Accept looks again and blocks the card
//! with a typed reason instead of pushing or merging partial work.

use std::path::PathBuf;
use std::sync::Arc;

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};

use super::Api;
use super::ships::{project, repo};
use crate::git::testing::{commit_file, git};
use crate::shipyard::GitShipyard;

struct Env {
    _tmp: tempfile::TempDir,
    repo: PathBuf,
    ship: PathBuf,
    api: Api,
}

/// A running card `DEMO-1` with a ship on a project whose `develop` has one commit.
async fn env() -> Env {
    let tmp = tempfile::tempdir().expect("tmp");
    let repo = repo(tmp.path());
    let api = Api::with_yard(Some(Arc::new(GitShipyard::new(tmp.path().join("ships")))));
    project(&api, &repo).await;
    api.post("/v1/chains/auth/tickets", json!({ "title": "Login" }))
        .await;
    let card = api.get("/v1/tickets/DEMO-1").await;
    let ship = PathBuf::from(card["ship"]["path"].as_str().expect("the card has a ship"));
    Env {
        _tmp: tmp,
        repo,
        ship,
        api,
    }
}

impl Env {
    async fn finish(&self) -> (StatusCode, Value) {
        self.api
            .call(
                Method::POST,
                "/v1/tickets/DEMO-1/events",
                Some(json!({ "event": "finished", "summary": "done" })),
            )
            .await
    }

    async fn card(&self) -> Value {
        self.api.get("/v1/tickets/DEMO-1").await
    }

    async fn accept(&self) -> Value {
        self.api
            .post("/v1/tickets/DEMO-1/actions/accept", json!({}))
            .await;
        self.card().await
    }

    /// The card is refused as not finished, says to commit, and is still Running.
    async fn assert_refused(&self, names: &[&str]) {
        let (status, body) = self.finish().await;
        assert_eq!(status, StatusCode::CONFLICT, "{body}");
        let text = body["error"].as_str().expect("an error text");
        assert!(text.contains("git commit"), "{text}");
        for name in names {
            assert!(text.contains(name), "{name} in {text}");
        }
        assert_eq!(self.api.status("DEMO-1").await, "running");
    }
}

fn reason(card: &Value) -> &Value {
    &card["ticket"]["status_reason"]["reason"]
}

#[tokio::test]
async fn finished_is_refused_while_a_tracked_file_has_uncommitted_changes() {
    let e = env().await;
    commit_file(&e.ship, "login.txt", "login\n", "add login");
    std::fs::write(e.ship.join("README.md"), "changed\n").expect("edit");
    e.assert_refused(&["README.md"]).await;

    git(&e.ship, &["commit", "--quiet", "-am", "edit readme"]);
    let (status, body) = e.finish().await;
    assert!(status.is_success(), "{body}");
    assert_eq!(e.api.status("DEMO-1").await, "review");
}

#[tokio::test]
async fn finished_is_refused_for_an_untracked_file_but_not_for_an_ignored_one() {
    let e = env().await;
    commit_file(&e.ship, ".gitignore", "target/\n", "ignore build output");
    std::fs::create_dir_all(e.ship.join("target")).expect("mkdir");
    std::fs::write(e.ship.join("target").join("out.bin"), "x").expect("ignored file");
    std::fs::write(e.ship.join("notes.txt"), "work\n").expect("untracked file");
    e.assert_refused(&["notes.txt"]).await;
    let (_, body) = e.finish().await;
    assert!(
        !body["error"].as_str().unwrap_or("").contains("out.bin"),
        "an ignored file is not work: {body}"
    );

    git(&e.ship, &["add", "notes.txt"]);
    git(&e.ship, &["commit", "--quiet", "-m", "notes"]);
    let (status, body) = e.finish().await;
    assert!(status.is_success(), "{body}");
}

#[tokio::test]
async fn finished_is_refused_when_the_branch_has_no_commit_of_its_own() {
    let e = env().await;
    e.assert_refused(&["no commit of its own", "develop"]).await;
    commit_file(&e.ship, "login.txt", "login\n", "add login");
    let (status, body) = e.finish().await;
    assert!(status.is_success(), "a clean ship with a commit: {body}");
}

#[tokio::test]
async fn accept_blocks_a_ship_that_became_dirty_and_pushes_and_merges_nothing() {
    let e = env().await;
    commit_file(&e.ship, "login.txt", "login\n", "add login");
    assert!(e.finish().await.0.is_success());
    let base = git(&e.repo, &["rev-parse", "develop"]);
    std::fs::write(e.ship.join("late.txt"), "late work\n").expect("untracked");

    let d = e.accept().await;
    assert_eq!(d["ticket"]["status"], "blocked", "{d}");
    assert_eq!(reason(&d)["kind"], "ship_uncommitted");
    assert_eq!(reason(&d)["files"], json!(["late.txt"]));
    assert_eq!(git(&e.repo, &["rev-parse", "develop"]), base, "not merged");
    assert!(e.ship.exists(), "the ship stays");

    // The inbox says it in words.
    let items = e.api.get("/v1/projects/DEMO/inbox").await;
    let item = &items.as_array().expect("list")[0];
    assert_eq!(item["kind"], "blocked");
    assert!(
        item["text"]
            .as_str()
            .is_some_and(|t| t.starts_with("ship_uncommitted: ") && t.contains("late.txt")),
        "{item}"
    );

    // Commit it on the ship, unblock: the merge goes through.
    git(&e.ship, &["add", "late.txt"]);
    git(&e.ship, &["commit", "--quiet", "-m", "late"]);
    e.api
        .post("/v1/tickets/DEMO-1/actions/unblock", json!({}))
        .await;
    assert_eq!(e.api.status("DEMO-1").await, "merged");
    assert!(e.repo.join("late.txt").is_file());
}

#[tokio::test]
async fn accept_blocks_a_ship_with_nothing_ahead_and_never_merges_it() {
    let e = env().await;
    commit_file(&e.ship, "login.txt", "login\n", "add login");
    assert!(e.finish().await.0.is_success());
    // The commits vanish from the branch after the hand-in.
    git(&e.ship, &["reset", "--hard", "--quiet", "develop"]);
    let base = git(&e.repo, &["rev-parse", "develop"]);

    let d = e.accept().await;
    assert_eq!(d["ticket"]["status"], "blocked", "{d}");
    assert_eq!(reason(&d)["kind"], "ship_empty");
    assert_eq!(git(&e.repo, &["rev-parse", "develop"]), base);
    let items = e.api.get("/v1/projects/DEMO/inbox").await;
    let text = items[0]["text"].as_str().expect("text");
    assert!(
        text.starts_with("ship_empty: ") && text.contains("no commit"),
        "{text}"
    );

    commit_file(&e.ship, "again.txt", "again\n", "again");
    e.api
        .post("/v1/tickets/DEMO-1/actions/unblock", json!({}))
        .await;
    assert_eq!(e.api.status("DEMO-1").await, "merged");
}

#[tokio::test]
async fn accept_of_a_clean_ship_with_commits_still_merges() {
    let e = env().await;
    commit_file(&e.ship, "login.txt", "login\n", "add login");
    assert!(e.finish().await.0.is_success());
    let d = e.accept().await;
    assert_eq!(d["ticket"]["status"], "merged", "{d}");
}

#[tokio::test]
async fn a_ticket_without_a_ship_is_not_held_by_the_guard() {
    let api = Api::new();
    api.demo().await;
    api.post("/v1/chains/demo/tickets", json!({ "title": "plain" }))
        .await;
    let (status, body) = api
        .call(
            Method::POST,
            "/v1/tickets/DEMO-1/events",
            Some(json!({ "event": "finished", "summary": "done" })),
        )
        .await;
    assert!(status.is_success(), "{body}");
}
