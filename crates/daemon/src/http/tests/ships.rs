//! Ships through the full router, with the git shipyard on a temporary repository: starting a
//! ticket makes the worktree and branch, Merged removes the worktree, a broken repository blocks
//! the ticket with a typed reason.

use std::path::Path;
use std::process::Command;
use std::sync::Arc;

use serde_json::{Value, json};

use super::Api;
use crate::shipyard::GitShipyard;

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .output()
        .expect("git runs");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

/// A repository with a `develop` branch and one commit.
fn repo(dir: &Path) -> std::path::PathBuf {
    let repo = dir.join("repo");
    std::fs::create_dir_all(&repo).expect("mkdir");
    git(&repo, &["init", "--quiet"]);
    git(&repo, &["symbolic-ref", "HEAD", "refs/heads/develop"]);
    std::fs::write(repo.join("README.md"), "hi\n").expect("write");
    git(&repo, &["add", "README.md"]);
    git(&repo, &["commit", "--quiet", "-m", "init"]);
    repo
}

async fn project(api: &Api, repo: &Path) {
    api.post(
        "/v1/projects",
        json!({ "key": "DEMO", "name": "Demo", "repo_path": repo.to_string_lossy() }),
    )
    .await;
    api.post("/v1/projects/DEMO/chains", json!({ "name": "auth" }))
        .await;
}

async fn journal(api: &Api) -> Vec<String> {
    let events = api.get("/v1/events").await;
    let events = events.as_array().expect("events");
    events
        .iter()
        .filter(|e| e["kind"].as_str().is_some_and(|k| k.starts_with("ship_")))
        .map(|e| e["kind"].as_str().expect("kind").to_owned())
        .collect()
}

#[tokio::test]
async fn start_makes_the_worktree_and_merged_removes_it_but_keeps_the_branch() {
    let tmp = tempfile::tempdir().expect("tmp");
    let repo = repo(tmp.path());
    let ships = tmp.path().join("home").join("ships");
    let api = Api::with_yard(Some(Arc::new(GitShipyard::new(ships.clone()))));
    project(&api, &repo).await;

    let d: Value = api
        .post("/v1/chains/auth/tickets", json!({ "title": "Login" }))
        .await;
    assert_eq!(d["ticket"]["status"], "running");
    // The answer is the committed state; the ship follows as its own event.
    let d = api.get("/v1/tickets/DEMO-1").await;
    let ship = &d["ship"];
    assert_eq!(ship["branch"], "ship/auth/demo-1-login");
    let path = Path::new(ship["path"].as_str().expect("path")).to_owned();
    assert_eq!(path, ships.join("demo").join("auth").join("demo-1-login"));
    assert!(path.join("README.md").is_file());
    assert_eq!(d["ticket"]["branch"], "ship/auth/demo-1-login");
    assert_eq!(
        git(&path, &["rev-parse", "--abbrev-ref", "HEAD"]),
        "ship/auth/demo-1-login"
    );
    assert_eq!(journal(&api).await, ["ship_created"]);

    api.post(
        "/v1/tickets/DEMO-1/events",
        json!({ "event": "finished", "summary": "done" }),
    )
    .await;
    assert!(
        path.exists(),
        "the worktree stays while the ticket is in review"
    );
    let d = api
        .post("/v1/tickets/DEMO-1/actions/accept", json!({}))
        .await;
    assert_eq!(d["ticket"]["status"], "merged");
    let d = api.get("/v1/tickets/DEMO-1").await;
    assert_eq!(d["ship"], Value::Null);
    assert!(!path.exists());
    assert!(
        git(&repo, &["branch", "--list", "ship/auth/demo-1-login"])
            .contains("ship/auth/demo-1-login")
    );
    assert_eq!(journal(&api).await, ["ship_created", "ship_removed"]);
}

#[tokio::test]
async fn a_ship_that_cannot_be_made_blocks_the_ticket_with_ship_failed() {
    let tmp = tempfile::tempdir().expect("tmp");
    let repo = repo(tmp.path());
    // A project whose base branch does not exist in the repository.
    let api = Api::with_yard(Some(Arc::new(GitShipyard::new(tmp.path().join("ships")))));
    api.post(
        "/v1/projects",
        json!({ "key": "DEMO", "repo_path": repo.to_string_lossy(), "work_branch": "nope" }),
    )
    .await;
    api.post("/v1/projects/DEMO/chains", json!({ "name": "auth" }))
        .await;

    let d = api
        .post("/v1/chains/auth/tickets", json!({ "title": "Login" }))
        .await;
    assert_eq!(
        d["ticket"]["status"], "running",
        "blocked only after the commit"
    );
    let d = api.get("/v1/tickets/DEMO-1").await;
    assert_eq!(d["ticket"]["status"], "blocked");
    assert_eq!(
        d["ticket"]["status_reason"]["reason"],
        json!({
            "kind": "ship_failed",
            "detail": "base branch nope does not exist in the repository",
        })
    );
    assert_eq!(d["ship"], Value::Null);
    assert!(journal(&api).await.is_empty());
}

#[tokio::test]
async fn a_project_without_a_repository_starts_tickets_without_ships() {
    let tmp = tempfile::tempdir().expect("tmp");
    let api = Api::with_yard(Some(Arc::new(GitShipyard::new(tmp.path().join("ships")))));
    api.demo().await;
    let d = api
        .post("/v1/chains/demo/tickets", json!({ "title": "A" }))
        .await;
    assert_eq!(d["ticket"]["status"], "running");
    assert_eq!(d["ship"], Value::Null);
    assert!(!tmp.path().join("ships").exists());
}

/// A git shipyard that notes which thread each call ran on.
#[derive(Debug)]
struct ThreadSpy {
    inner: GitShipyard,
    threads: Arc<std::sync::Mutex<Vec<Option<String>>>>,
}

impl ThreadSpy {
    fn note(&self) {
        let name = std::thread::current().name().map(str::to_owned);
        self.threads.lock().expect("lock").push(name);
    }
}

impl toknila_app::Shipyard for ThreadSpy {
    fn prepare(&self, spec: &toknila_core::ShipSpec) -> Result<String, toknila_app::ShipError> {
        self.note();
        self.inner.prepare(spec)
    }

    fn remove(&self, repo: &str, ship: &toknila_core::Ship) -> Result<(), toknila_app::ShipError> {
        self.note();
        self.inner.remove(repo, ship)
    }
}

#[tokio::test]
async fn git_runs_after_the_commit_and_never_on_the_store_thread() {
    let tmp = tempfile::tempdir().expect("tmp");
    let repo = repo(tmp.path());
    let threads = Arc::new(std::sync::Mutex::new(Vec::new()));
    let spy = ThreadSpy {
        inner: GitShipyard::new(tmp.path().join("ships")),
        threads: threads.clone(),
    };
    let api = Api::with_yard(Some(Arc::new(spy)));
    project(&api, &repo).await;
    api.post("/v1/chains/auth/tickets", json!({ "title": "Login" }))
        .await;
    let seen = threads.lock().expect("lock").clone();
    assert_eq!(seen.len(), 1, "one prepare: {seen:?}");
    assert_ne!(
        seen[0].as_deref(),
        Some("toknila-store"),
        "git ran on the store's transaction thread"
    );
}

#[tokio::test]
async fn a_worktree_that_cannot_be_removed_is_journaled_and_the_accept_stands() {
    let tmp = tempfile::tempdir().expect("tmp");
    let repo = repo(tmp.path());
    let api = Api::with_yard(Some(Arc::new(GitShipyard::new(tmp.path().join("ships")))));
    project(&api, &repo).await;
    api.post("/v1/chains/auth/tickets", json!({ "title": "Login" }))
        .await;
    api.post(
        "/v1/tickets/DEMO-1/events",
        json!({ "event": "finished", "summary": "done" }),
    )
    .await;
    // The repository disappears under the ship: git cannot remove the worktree from it.
    std::fs::remove_dir_all(&repo).expect("delete the repository");
    let d = api
        .post("/v1/tickets/DEMO-1/actions/accept", json!({}))
        .await;
    assert_eq!(d["ticket"]["status"], "merged");
    assert_eq!(journal(&api).await, ["ship_created", "ship_remove_failed"]);
    let d = api.get("/v1/tickets/DEMO-1").await;
    assert_ne!(d["ship"], Value::Null, "the path stays recorded");
}
