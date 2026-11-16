//! The built-artifact guard through the full router with the real git shipyard: `finished` is
//! refused when the ship's commits add a build output or a file the repository ignores, naming
//! each and saying how to remove it; a path the card's prompt names passes.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};

use super::Api;
use super::ships::{project, repo};
use crate::git::testing::{commit_file, git};
use crate::shipyard::GitShipyard;

const SENTENCE: &str =
    "these are built or ignored files; remove them from git (git rm --cached) and hand in again";

struct Env {
    _tmp: tempfile::TempDir,
    ship: PathBuf,
    api: Api,
}

/// A running card `DEMO-1` (with `prompt`) on a ship of a project whose `develop` has one commit.
async fn env(prompt: &str) -> Env {
    let tmp = tempfile::tempdir().expect("tmp");
    let repo = repo(tmp.path());
    let api = Api::with_yard(Some(Arc::new(GitShipyard::new(tmp.path().join("ships")))));
    project(&api, &repo).await;
    api.post(
        "/v1/chains/auth/tickets",
        json!({ "title": "Login", "prompt": prompt }),
    )
    .await;
    let card = api.get("/v1/tickets/DEMO-1").await;
    let ship = PathBuf::from(card["ship"]["path"].as_str().expect("the card has a ship"));
    Env {
        _tmp: tmp,
        ship,
        api,
    }
}

/// Writes `name` (with its folders) in the ship and commits it, forcing past any ignore rule.
fn commit_path(dir: &Path, name: &str) {
    let path = dir.join(name);
    std::fs::create_dir_all(path.parent().expect("a parent")).expect("mkdir");
    std::fs::write(&path, "x\n").expect("write");
    git(dir, &["add", "-f", name]);
    git(dir, &["commit", "--quiet", "-m", name]);
}

impl Env {
    async fn finish(&self) -> (StatusCode, Value) {
        self.api
            .call(
                Method::POST,
                "/v1/tickets/DEMO-1/events",
                Some(json!({ "event": "finished", "summary": toknila_core::handin::from_plain("done") })),
            )
            .await
    }

    async fn assert_refused(&self, names: &[&str]) {
        let (status, body) = self.finish().await;
        assert_eq!(status, StatusCode::CONFLICT, "{body}");
        let text = body["error"].as_str().expect("an error text");
        assert!(text.contains(SENTENCE), "{text}");
        for name in names {
            assert!(text.contains(name), "{name} in {text}");
        }
        assert_eq!(self.api.status("DEMO-1").await, "running");
    }
}

#[tokio::test]
async fn finished_is_refused_for_a_committed_pycache_file_and_passes_once_it_is_removed() {
    let e = env("Add the login page.").await;
    commit_file(&e.ship, "login.txt", "login\n", "add login");
    commit_path(&e.ship, "x/__pycache__/a.pyc");
    e.assert_refused(&["x/__pycache__/a.pyc"]).await;

    git(
        &e.ship,
        &["rm", "--cached", "--quiet", "x/__pycache__/a.pyc"],
    );
    git(&e.ship, &["commit", "--quiet", "-m", "remove the cache"]);
    // `git rm --cached` leaves the file on disk; the agent deletes it (or ignores it).
    std::fs::remove_dir_all(e.ship.join("x")).expect("delete the cache");
    let (status, body) = e.finish().await;
    assert!(status.is_success(), "{body}");
    assert_eq!(e.api.status("DEMO-1").await, "review");
}

#[tokio::test]
async fn finished_names_every_artifact_and_a_file_the_repository_ignores() {
    let e = env("Add the login page.").await;
    commit_file(
        &e.ship,
        ".gitignore",
        "secrets.local\n",
        "ignore local secrets",
    );
    commit_path(&e.ship, "secrets.local");
    commit_path(&e.ship, "dist/app.js");
    commit_path(&e.ship, "run.log");
    e.assert_refused(&["secrets.local", "dist/app.js", "run.log"])
        .await;
}

/// Git itself answers the `.gitignore` question, tracked files included, and a file the ship's
/// own commits add is listed once and only when it is still there.
#[tokio::test]
async fn the_yard_asks_git_what_is_ignored_and_lists_only_added_files() {
    use toknila_app::Shipyard;
    use toknila_core::ShipLook;

    let e = env("Add the login page.").await;
    let start = git(&e.ship, &["rev-parse", "HEAD"]);
    let look = ShipLook {
        path: e.ship.to_string_lossy().into_owned(),
        start: start.clone(),
    };
    commit_file(&e.ship, ".gitignore", "*.secret\nout/\n", "ignore");
    commit_path(&e.ship, "a.secret");
    commit_path(&e.ship, "out/b.txt");
    commit_path(&e.ship, "src/c.rs");
    commit_path(&e.ship, "gone.txt");
    git(&e.ship, &["rm", "--quiet", "gone.txt"]);
    git(&e.ship, &["commit", "--quiet", "-m", "drop gone"]);
    std::fs::write(e.ship.join("src/c.rs"), "changed\n").expect("edit");
    git(&e.ship, &["commit", "--quiet", "-am", "change c"]);

    let yard = GitShipyard::new(e._tmp.path().join("ships"));
    let added = yard.added_between(&look, &start, "HEAD").expect("added");
    let mut sorted = added.clone();
    sorted.sort();
    assert_eq!(sorted, [".gitignore", "a.secret", "out/b.txt", "src/c.rs"]);
    let mut ignored = yard.ignored_among(&look, &added).expect("ignored");
    ignored.sort();
    assert_eq!(ignored, ["a.secret", "out/b.txt"]);
    assert!(yard.ignored_among(&look, &[]).expect("none").is_empty());
}

#[tokio::test]
async fn a_path_the_prompt_names_is_let_through() {
    let e = env("Add a fixture under tests/fixtures/dist/ for the packer.").await;
    commit_path(&e.ship, "tests/fixtures/dist/a.txt");
    let (status, body) = e.finish().await;
    assert!(status.is_success(), "{body}");

    // The prompt names that path only: another artifact is still refused after a Return.
    e.api
        .post(
            "/v1/tickets/DEMO-1/actions/return",
            json!({ "text": "again" }),
        )
        .await;
    commit_path(&e.ship, "x/__pycache__/a.pyc");
    e.assert_refused(&["x/__pycache__/a.pyc"]).await;
}
