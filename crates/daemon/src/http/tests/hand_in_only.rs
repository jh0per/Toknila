//! Nothing lands that was not handed in, through the full router with the real git shipyard and
//! merger: Accept refuses commits beyond the hand-in (the daemon's own base-sync merge is not
//! one), and a hand-in after a Return keeps what changed since it.

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
    async fn finish(&self) {
        let (status, body) = self
            .api
            .call(
                Method::POST,
                "/v1/tickets/DEMO-1/events",
                Some(json!({ "event": "finished", "summary": toknila_core::handin::from_plain("done") })),
            )
            .await;
        assert!(status.is_success(), "{body}");
    }

    async fn accept(&self) -> (StatusCode, Value) {
        self.api
            .call(
                Method::POST,
                "/v1/tickets/DEMO-1/actions/accept",
                Some(json!({})),
            )
            .await
    }

    async fn review(&self) -> Value {
        self.api.get("/v1/tickets/DEMO-1").await["ticket"]["review"].clone()
    }
}

#[tokio::test]
async fn accept_refuses_a_commit_beyond_the_hand_in_and_names_it() {
    let e = env().await;
    commit_file(&e.ship, "login.txt", "login\n", "add login");
    e.finish().await;
    let handed_in = git(&e.ship, &["rev-parse", "HEAD"]);
    commit_file(&e.ship, "late.txt", "late\n", "late work");
    let late = git(&e.ship, &["rev-parse", "HEAD"]);
    let base = git(&e.repo, &["rev-parse", "develop"]);

    let (status, body) = e.accept().await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    let text = body["error"].as_str().expect("an error text");
    for want in [
        &late[..7],
        "late work",
        &handed_in[..7],
        "Return the card so the agent hands in again",
        "wait for a new hand-in",
    ] {
        assert!(text.contains(want), "{want} in {text}");
    }
    assert!(
        !text.contains("add login"),
        "the hand-in is not extra: {text}"
    );
    assert_eq!(e.api.status("DEMO-1").await, "review", "the card stays put");
    assert_eq!(git(&e.repo, &["rev-parse", "develop"]), base, "not merged");

    // A new hand-in takes the commit in: Accept goes through.
    e.api
        .post(
            "/v1/tickets/DEMO-1/actions/return",
            json!({ "text": "again" }),
        )
        .await;
    commit_file(&e.ship, "more.txt", "more\n", "more work");
    e.finish().await;
    let (status, body) = e.accept().await;
    assert!(status.is_success(), "{body}");
    assert_eq!(e.api.status("DEMO-1").await, "merged");
    assert!(e.repo.join("late.txt").is_file());
}

#[tokio::test]
async fn the_daemons_own_base_sync_merge_is_recorded_and_not_an_extra_commit() {
    let e = env().await;
    commit_file(&e.repo, "base.txt", "base\n", "base moves");
    commit_file(&e.ship, "login.txt", "login\n", "add login");
    e.finish().await;

    // The hand-in was at the ship's commit; the daemon then merged the base into it.
    let review = e.review().await;
    let handed_in = review["handed_in"].as_str().expect("handed in").to_owned();
    let head = git(&e.ship, &["rev-parse", "HEAD"]);
    assert_ne!(head, handed_in, "the sync made a merge commit on top");
    assert_eq!(review["synced"], json!([head]), "the sync recorded it");

    let (status, body) = e.accept().await;
    assert!(status.is_success(), "{body}");
    assert_eq!(e.api.status("DEMO-1").await, "merged");
}

#[tokio::test]
async fn a_commit_after_the_sync_is_refused_and_the_sync_merge_is_not_named() {
    let e = env().await;
    commit_file(&e.repo, "base.txt", "base\n", "base moves");
    commit_file(&e.ship, "login.txt", "login\n", "add login");
    e.finish().await;
    commit_file(&e.ship, "late.txt", "late\n", "late work");

    let (status, body) = e.accept().await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    let text = body["error"].as_str().expect("an error text");
    assert!(text.contains("late work"), "{text}");
    assert!(
        !text.contains("Merge"),
        "the sync merge is not named: {text}"
    );
    assert!(
        !text.contains("base moves"),
        "base commits are not named: {text}"
    );
}

#[tokio::test]
async fn a_hand_in_after_a_return_keeps_what_changed_since_it() {
    let e = env().await;
    commit_file(&e.ship, "login.txt", "login\n", "add login");
    e.finish().await;
    assert_eq!(
        e.review().await["since_return"],
        Value::Null,
        "no Return yet"
    );
    let first = e.review().await["handed_in"]
        .as_str()
        .expect("head")
        .to_owned();

    e.api
        .post(
            "/v1/tickets/DEMO-1/actions/return",
            json!({ "text": "rename the flag" }),
        )
        .await;
    commit_file(&e.ship, "flag.txt", "flag\n", "rename flag");
    commit_file(&e.ship, "login.txt", "login v2\n", "fix login");
    let second = git(&e.ship, &["rev-parse", "HEAD"]);
    e.finish().await;

    let review = e.review().await;
    let since = &review["since_return"];
    // The round that was returned, not the check's round that followed the new hand-in.
    assert_eq!(since["round"], 2, "{review}");
    assert_eq!(
        review["rounds"][1]["verdict"], "request_changes",
        "{review}"
    );
    assert_eq!(since["from"], json!(first));
    assert_eq!(since["to"], json!(second));
    assert_eq!(
        since["commits"],
        json!(["fix login", "rename flag"]),
        "newest first"
    );
    assert_eq!(since["files"], json!(["flag.txt", "login.txt"]));
    assert!(since.get("more_commits").is_none() && since.get("more_files").is_none());
}
