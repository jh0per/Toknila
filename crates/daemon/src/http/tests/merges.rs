//! Accept through the full router with the real git shipyard and merger on a temporary
//! repository: the card is merged into `develop`, a conflict or a dirty checkout blocks it with a
//! typed reason, unblocking after a fix merges, and a dependent card starts from the new base.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{Value, json};

use super::Api;
use super::ships::{project, repo};
use crate::git::testing::{commit_file, git};
use crate::shipyard::GitShipyard;

struct Env {
    _tmp: tempfile::TempDir,
    repo: PathBuf,
    api: Api,
}

async fn env() -> Env {
    let tmp = tempfile::tempdir().expect("tmp");
    let repo = repo(tmp.path());
    let api = Api::with_yard(Some(Arc::new(GitShipyard::new(tmp.path().join("ships")))));
    project(&api, &repo).await;
    Env {
        _tmp: tmp,
        repo,
        api,
    }
}

async fn add(api: &Api, body: Value) {
    api.post("/v1/chains/auth/tickets", body).await;
}

async fn card(api: &Api, key: &str) -> Value {
    api.get(&format!("/v1/tickets/{key}")).await
}

fn ship_path(card: &Value) -> PathBuf {
    PathBuf::from(card["ship"]["path"].as_str().expect("the card has a ship"))
}

async fn hand_in_and_accept(api: &Api, key: &str) {
    api.post(
        &format!("/v1/tickets/{key}/events"),
        json!({ "event": "finished", "summary": toknila_core::handin::from_plain("done") }),
    )
    .await;
    api.post(&format!("/v1/tickets/{key}/actions/accept"), json!({}))
        .await;
}

fn status(card: &Value) -> &str {
    card["ticket"]["status"].as_str().expect("status")
}

fn reason(card: &Value) -> &Value {
    &card["ticket"]["status_reason"]["reason"]
}

fn tracked(dir: &Path, rev: &str) -> String {
    git(dir, &["ls-tree", "-r", "--name-only", rev])
}

#[tokio::test]
async fn accept_merges_the_ship_into_develop_with_a_merge_commit_and_keeps_the_checkout_clean() {
    let e = env().await;
    add(&e.api, json!({ "title": "Add login" })).await;
    let d = card(&e.api, "DEMO-1").await;
    let ship = ship_path(&d);
    commit_file(&ship, "login.txt", "login\n", "add login");
    let before = git(&e.repo, &["rev-parse", "develop"]);

    hand_in_and_accept(&e.api, "DEMO-1").await;

    let d = card(&e.api, "DEMO-1").await;
    assert_eq!(status(&d), "merged");
    let parents = git(&e.repo, &["rev-list", "--parents", "-n", "1", "develop"]);
    let parents: Vec<&str> = parents.split(' ').collect();
    assert_eq!(parents.len(), 3, "two parents: {parents:?}");
    assert_eq!(parents[1], before);
    assert_eq!(
        git(&e.repo, &["log", "-1", "--format=%s", "develop"]),
        "Merge ship/auth/demo-1-add-login: DEMO-1 Add login"
    );
    assert!(e.repo.join("login.txt").is_file(), "the checkout has it");
    assert_eq!(git(&e.repo, &["status", "--porcelain"]), "");
    assert!(!ship.exists(), "the worktree is removed");
    assert!(git(&e.repo, &["branch", "--list", "ship/auth/*"]).contains("demo-1-add-login"));
}

#[tokio::test]
async fn a_conflict_met_at_the_hand_in_returns_the_card_to_the_worker_and_the_fix_merges() {
    let e = env().await;
    add(&e.api, json!({ "title": "Edit readme" })).await;
    let ship = ship_path(&card(&e.api, "DEMO-1").await);
    commit_file(&ship, "README.md", "ship says hi\n", "ship edit");
    commit_file(&e.repo, "README.md", "base says hi\n", "base edit");

    e.api
        .post(
            "/v1/tickets/DEMO-1/events",
            json!({ "event": "finished", "summary": toknila_core::handin::from_plain("done") }),
        )
        .await;

    let d = card(&e.api, "DEMO-1").await;
    assert_eq!(status(&d), "running", "{d}");
    let comments = d["comments"].as_array().expect("comments");
    let last = comments.last().expect("an instruction was sent");
    assert_eq!(last["kind"], "instruction", "{d}");
    assert!(last["body"].to_string().contains("README.md"), "{d}");
    assert!(
        d["questions"].as_array().is_none_or(|q| q.is_empty()),
        "nobody is asked: {d}"
    );

    // The worker resolves it in the ship and hands in again.
    git(&ship, &["merge", "--no-edit", "-s", "ours", "develop"]);
    hand_in_and_accept(&e.api, "DEMO-1").await;
    let d = card(&e.api, "DEMO-1").await;
    assert_eq!(status(&d), "merged", "{d}");
    assert_eq!(git(&e.repo, &["show", "develop:README.md"]), "ship says hi");
    assert_eq!(git(&e.repo, &["status", "--porcelain"]), "");
}

#[tokio::test]
async fn a_conflict_met_at_accept_blocks_with_the_file_names_and_unblock_after_the_fix_merges() {
    let e = env().await;
    add(&e.api, json!({ "title": "Edit readme" })).await;
    let ship = ship_path(&card(&e.api, "DEMO-1").await);
    commit_file(&ship, "README.md", "ship says hi\n", "ship edit");
    // Hand in while the base has not moved: the check passes and the card waits for Accept.
    e.api
        .post(
            "/v1/tickets/DEMO-1/events",
            json!({ "event": "finished", "summary": toknila_core::handin::from_plain("done") }),
        )
        .await;
    commit_file(&e.repo, "README.md", "base says hi\n", "base edit");
    let base = git(&e.repo, &["rev-parse", "develop"]);
    e.api
        .post("/v1/tickets/DEMO-1/actions/accept", json!({}))
        .await;

    let d = card(&e.api, "DEMO-1").await;
    assert_eq!(status(&d), "blocked");
    assert_eq!(reason(&d)["kind"], "merge_conflict");
    assert!(
        reason(&d)["detail"]
            .as_str()
            .expect("detail")
            .contains("README.md"),
        "{d}"
    );
    assert_eq!(git(&e.repo, &["rev-parse", "develop"]), base, "untouched");
    assert!(
        ship.exists(),
        "the ship stays so the conflict can be fixed there"
    );

    // The person (or the agent) resolves it on the ship, then unblocks.
    git(&ship, &["merge", "--no-edit", "-s", "ours", "develop"]);
    e.api
        .post("/v1/tickets/DEMO-1/actions/unblock", json!({}))
        .await;
    let d = card(&e.api, "DEMO-1").await;
    assert_eq!(status(&d), "merged");
    assert_eq!(git(&e.repo, &["show", "develop:README.md"]), "ship says hi");
    assert_eq!(git(&e.repo, &["status", "--porcelain"]), "");
}

#[tokio::test]
async fn a_dirty_base_checkout_blocks_and_nothing_changes_until_it_is_cleaned() {
    let e = env().await;
    add(&e.api, json!({ "title": "Add login" })).await;
    let ship = ship_path(&card(&e.api, "DEMO-1").await);
    commit_file(&ship, "login.txt", "login\n", "add login");
    std::fs::write(e.repo.join("README.md"), "work in progress\n").expect("write");
    let base = git(&e.repo, &["rev-parse", "develop"]);

    hand_in_and_accept(&e.api, "DEMO-1").await;

    let d = card(&e.api, "DEMO-1").await;
    assert_eq!(status(&d), "blocked");
    assert_eq!(reason(&d)["kind"], "base_checkout_dirty");
    let named = PathBuf::from(reason(&d)["path"].as_str().expect("path"));
    assert_eq!(
        std::fs::canonicalize(named).expect("path exists"),
        std::fs::canonicalize(&e.repo).expect("repo exists")
    );
    assert_eq!(git(&e.repo, &["rev-parse", "develop"]), base);
    assert_eq!(
        std::fs::read_to_string(e.repo.join("README.md")).expect("read"),
        "work in progress\n"
    );

    git(&e.repo, &["checkout", "--", "README.md"]);
    e.api
        .post("/v1/tickets/DEMO-1/actions/unblock", json!({}))
        .await;
    assert_eq!(status(&card(&e.api, "DEMO-1").await), "merged");
    assert!(e.repo.join("login.txt").is_file());
}

#[tokio::test]
async fn a_dependent_card_started_after_the_merge_branches_from_the_new_base() {
    let e = env().await;
    add(&e.api, json!({ "title": "Add login" })).await;
    add(&e.api, json!({ "title": "Use login", "after": ["DEMO-1"] })).await;
    let ship = ship_path(&card(&e.api, "DEMO-1").await);
    commit_file(&ship, "login.txt", "login\n", "add login");
    assert_eq!(status(&card(&e.api, "DEMO-2").await), "backlog");

    hand_in_and_accept(&e.api, "DEMO-1").await;

    let second = card(&e.api, "DEMO-2").await;
    assert_eq!(status(&second), "running");
    let ship2 = ship_path(&second);
    assert!(ship2.join("login.txt").is_file(), "it sees the merged work");
    assert_eq!(
        git(&ship2, &["rev-parse", "HEAD"]),
        git(&e.repo, &["rev-parse", "develop"]),
        "it starts exactly at the new base"
    );
    assert!(tracked(&ship2, "HEAD").contains("login.txt"));
}
