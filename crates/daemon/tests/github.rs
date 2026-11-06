//! Pull-request mode through the full router with the real git shipyard, a bare local repository
//! as `origin`, and the workspace's `fake-gh` first on `PATH` (no network, no account). Checks are
//! moved along by editing the fake's state file, and the daemon's ticker is run by hand.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Method, Request};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tokio::sync::Notify;
use toknila_daemon::github::{GithubMerger, Mergers};
use toknila_daemon::server::{AppState, router};
use toknila_daemon::shipyard::GitShipyard;
use toknila_daemon::store::Store;
use tower::ServiceExt;

const FAKE_GH: &str = env!("CARGO_BIN_EXE_fake-gh");
const TOKEN: &str = "t";
const BRANCH: &str = "ship/auth/demo-1-add-login";

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

fn commit_file(dir: &Path, name: &str, content: &str) {
    std::fs::write(dir.join(name), content).expect("write");
    git(dir, &["add", name]);
    git(dir, &["commit", "--quiet", "-m", name]);
}

struct Rig {
    _tmp: tempfile::TempDir,
    repo: PathBuf,
    origin: PathBuf,
    state: PathBuf,
    store: Store,
    app: axum::Router,
}

impl Rig {
    async fn new() -> Rig {
        let tmp = tempfile::tempdir().expect("tmp");
        let origin = tmp.path().join("origin.git");
        git(tmp.path(), &["init", "--quiet", "--bare", "origin.git"]);
        git(&origin, &["symbolic-ref", "HEAD", "refs/heads/develop"]);
        let repo = tmp.path().join("repo");
        std::fs::create_dir_all(&repo).expect("mkdir");
        git(&repo, &["init", "--quiet"]);
        git(&repo, &["symbolic-ref", "HEAD", "refs/heads/develop"]);
        commit_file(&repo, "README.md", "hi\n");
        git(
            &repo,
            &["remote", "add", "origin", &origin.to_string_lossy()],
        );
        git(&repo, &["push", "--quiet", "origin", "develop"]);

        // The fake goes first on PATH under the name `gh`, as the real one would be found.
        let bin = tmp.path().join("bin");
        std::fs::create_dir_all(&bin).expect("mkdir");
        std::fs::copy(
            FAKE_GH,
            bin.join(format!("gh{}", std::env::consts::EXE_SUFFIX)),
        )
        .expect("copy the fake gh");
        let old = std::env::var_os("PATH").unwrap_or_default();
        let path = std::env::join_paths(std::iter::once(bin).chain(std::env::split_paths(&old)))
            .expect("PATH");
        let state = tmp.path().join("gh-state.json");
        std::fs::write(
            &state,
            json!({ "logged_in": true, "origin": origin, "prs": [], "calls": [],
                    "new_checks": [{ "name": "verify", "bucket": "pending" }] })
            .to_string(),
        )
        .expect("state");
        let gh = GithubMerger::new()
            .with_env("PATH", &path.to_string_lossy())
            .with_env("FAKE_GH_STATE", &state.to_string_lossy());
        let store = Store::open(&tmp.path().join("t.db"))
            .expect("store")
            .with_shipyard(Arc::new(GitShipyard::new(tmp.path().join("ships"))))
            .with_merger(Arc::new(Mergers::with_github(gh)));
        let (runners, _) =
            toknila_daemon::runner::Runners::new(tmp.path().join("runs"), tmp.path().join("p"));
        let app = router(AppState {
            store: store.clone(),
            token: TOKEN.into(),
            shutdown: Arc::new(Notify::new()),
            profiles: tmp.path().join("p"),
            runners,
            tools: Arc::new(toknila_daemon::tools::SystemTools),
            usage: Arc::new(toknila_daemon::usage::Usage::disabled()),
        });
        let rig = Rig {
            _tmp: tmp,
            repo,
            origin,
            state,
            store,
            app,
        };
        rig.post(
            "/v1/projects",
            json!({ "key": "DEMO", "repo_path": rig.repo.to_string_lossy() }),
        )
        .await;
        rig.post("/v1/projects/DEMO/chains", json!({ "name": "auth" }))
            .await;
        rig.call(
            Method::PATCH,
            "/v1/projects/DEMO",
            json!({ "merge_mode": "github", "merge_method": "squash" }),
        )
        .await;
        rig
    }

    async fn call(&self, method: Method, uri: &str, body: Value) -> Value {
        let req = Request::builder()
            .method(method)
            .uri(uri)
            .header("authorization", format!("Bearer {TOKEN}"))
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .expect("request");
        let resp = self.app.clone().oneshot(req).await.expect("response");
        let ok = resp.status().is_success();
        let bytes = resp.into_body().collect().await.expect("body").to_bytes();
        let v: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        assert!(ok, "{uri}: {v}");
        v
    }

    async fn post(&self, uri: &str, body: Value) -> Value {
        self.call(Method::POST, uri, body).await
    }

    async fn get(&self, uri: &str) -> Value {
        self.call(Method::GET, uri, Value::Null).await
    }

    async fn card(&self, key: &str) -> Value {
        self.get(&format!("/v1/tickets/{key}")).await
    }

    fn gh(&self) -> Value {
        serde_json::from_str(&std::fs::read_to_string(&self.state).expect("state")).expect("json")
    }

    /// Edits the fake's state, as GitHub would change on its own.
    fn gh_edit(&self, f: impl FnOnce(&mut Value)) {
        let mut v = self.gh();
        f(&mut v);
        std::fs::write(&self.state, v.to_string()).expect("write state");
    }

    fn checks(&self, bucket: &str) {
        self.gh_edit(|s| s["prs"][0]["checks"] = json!([{ "name": "verify", "bucket": bucket }]));
    }

    fn creates(&self) -> usize {
        let calls = self.gh()["calls"].clone();
        calls
            .as_array()
            .expect("calls")
            .iter()
            .filter(|c| c[1] == "create")
            .count()
    }

    /// The first card, with a commit on its ship, handed in and accepted.
    async fn accepted(&self) -> PathBuf {
        self.post(
            "/v1/chains/auth/tickets",
            json!({ "title": "Add login", "prompt": "Let people in.", "acceptance": ["It logs in"] }),
        )
        .await;
        let ship = PathBuf::from(
            self.card("DEMO-1").await["ship"]["path"]
                .as_str()
                .expect("ship"),
        );
        commit_file(&ship, "login.txt", "login\n");
        self.post(
            "/v1/tickets/DEMO-1/events",
            json!({ "event": "finished", "summary": "done" }),
        )
        .await;
        self.post("/v1/tickets/DEMO-1/actions/accept", json!({}))
            .await;
        ship
    }

    async fn tick(&self) {
        toknila_daemon::pr_watch::tick(&self.store).await;
    }

    async fn status(&self) -> String {
        self.card("DEMO-1").await["ticket"]["status"]
            .as_str()
            .expect("status")
            .to_owned()
    }

    fn origin_has(&self, rev: &str, file: &str) -> bool {
        git(&self.origin, &["ls-tree", "-r", "--name-only", rev]).contains(file)
    }
}

fn reason(card: &Value) -> &Value {
    &card["ticket"]["status_reason"]["reason"]
}

#[tokio::test]
async fn green_checks_merge_the_pull_request_and_new_ships_start_from_origin() {
    let rig = Rig::new().await;
    let ship = rig.accepted().await;

    // Accept pushed the branch and opened the pull request from the template and the card.
    let d = rig.card("DEMO-1").await;
    assert_eq!(d["ticket"]["status"], "review");
    assert_eq!(d["ticket"]["review"]["turn"]["by"], "merging");
    assert_eq!(d["ticket"]["pr"]["url"], "https://github.com/o/r/pull/1");
    assert_eq!(d["ticket"]["pr"]["checks"][0]["state"], "pending");
    assert!(rig.origin_has(BRANCH, "login.txt"), "the branch was pushed");
    let pr = &rig.gh()["prs"][0];
    assert_eq!(
        (pr["title"].as_str(), pr["base"].as_str()),
        (Some("DEMO-1: Add login"), Some("develop"))
    );
    let body = pr["body"].as_str().expect("body");
    assert!(
        body.contains("Let people in.")
            && body.contains("- It logs in")
            && body.contains("person approved"),
        "{body}"
    );

    // Pending checks keep it waiting.
    rig.tick().await;
    assert_eq!(rig.status().await, "review");

    rig.checks("pass");
    rig.tick().await;
    let d = rig.card("DEMO-1").await;
    assert_eq!(d["ticket"]["status"], "merged");
    assert_eq!(d["ticket"]["pr"]["checks"][0]["state"], "passed");
    assert_eq!(
        rig.gh()["prs"][0]["method"],
        "--squash",
        "the project's method"
    );
    assert!(
        rig.origin_has("develop", "login.txt"),
        "origin's base has it"
    );
    assert_eq!(
        git(&rig.repo, &["rev-parse", "origin/develop"]),
        git(&rig.origin, &["rev-parse", "develop"]),
        "origin was fetched"
    );
    assert!(!ship.exists(), "the worktree is removed");

    // The next ship branches from origin/develop; the local develop never got the merge.
    rig.post("/v1/chains/auth/tickets", json!({ "title": "Next" }))
        .await;
    let next = PathBuf::from(
        rig.card("DEMO-2").await["ship"]["path"]
            .as_str()
            .expect("ship"),
    );
    assert!(next.join("login.txt").is_file());
    assert!(!git(&rig.repo, &["ls-tree", "-r", "--name-only", "develop"]).contains("login.txt"));
}

#[tokio::test]
async fn failing_checks_block_with_the_link_and_a_fix_then_unblock_merges_the_same_pull_request() {
    let rig = Rig::new().await;
    let ship = rig.accepted().await;
    rig.checks("fail");
    rig.tick().await;

    let d = rig.card("DEMO-1").await;
    assert_eq!(d["ticket"]["status"], "blocked");
    assert_eq!(reason(&d)["kind"], "ci_failed");
    assert_eq!(reason(&d)["url"], "https://github.com/o/r/pull/1");
    assert_eq!(reason(&d)["checks"], json!(["verify"]));
    assert_eq!(d["ticket"]["pr"]["checks"][0]["state"], "failed");
    let inbox = rig.get("/v1/projects/DEMO/inbox").await;
    assert!(
        inbox
            .as_array()
            .expect("inbox")
            .iter()
            .any(|i| i["kind"] == "blocked"
                && i["text"]
                    .as_str()
                    .is_some_and(|t| t.contains("ci_failed") && t.contains("pull/1"))),
        "{inbox}"
    );

    // The agent fixes it on the ship; CI starts again on GitHub's side; unblock pushes the fix.
    commit_file(&ship, "fix.txt", "fix\n");
    rig.checks("pending");
    rig.post("/v1/tickets/DEMO-1/actions/unblock", json!({}))
        .await;
    assert_eq!(rig.status().await, "review");
    assert!(rig.origin_has(BRANCH, "fix.txt"), "unblock pushed the fix");
    assert_eq!(rig.creates(), 1, "the pull request is the same one");

    rig.checks("pass");
    rig.tick().await;
    assert_eq!(rig.status().await, "merged");
    assert!(rig.origin_has("develop", "fix.txt"));
}

#[tokio::test]
async fn a_pull_request_that_exists_for_the_branch_is_reused() {
    let rig = Rig::new().await;
    rig.gh_edit(|s| {
        s["prs"] = json!([{ "number": 41, "url": "https://github.com/o/r/pull/41", "branch": BRANCH,
            "base": "develop", "title": "mine", "body": "", "state": "OPEN",
            "mergeable": "MERGEABLE", "checks": [{ "name": "verify", "bucket": "pending" }] }]);
    });
    rig.accepted().await;
    let d = rig.card("DEMO-1").await;
    assert_eq!(d["ticket"]["pr"]["number"], 41);
    assert_eq!(rig.creates(), 0, "nothing was created");
}

#[tokio::test]
async fn gh_not_logged_in_is_a_typed_block_with_the_fix_and_unblock_goes_on() {
    let rig = Rig::new().await;
    rig.gh_edit(|s| s["logged_in"] = json!(false));
    rig.accepted().await;
    let d = rig.card("DEMO-1").await;
    assert_eq!(d["ticket"]["status"], "blocked");
    assert_eq!(reason(&d)["kind"], "merge_failed");
    assert!(
        reason(&d)["detail"]
            .as_str()
            .expect("detail")
            .contains("gh auth login"),
        "{d}"
    );
    rig.gh_edit(|s| s["logged_in"] = json!(true));
    rig.post("/v1/tickets/DEMO-1/actions/unblock", json!({}))
        .await;
    let d = rig.card("DEMO-1").await;
    assert_eq!(d["ticket"]["status"], "review");
    assert_eq!(d["ticket"]["pr"]["number"], 1);
}

#[tokio::test]
async fn a_pull_request_closed_or_in_conflict_on_github_blocks_with_the_link() {
    let rig = Rig::new().await;
    rig.accepted().await;
    rig.gh_edit(|s| s["prs"][0]["state"] = json!("CLOSED"));
    rig.tick().await;
    let d = rig.card("DEMO-1").await;
    assert_eq!(reason(&d)["kind"], "pr_closed");
    assert_eq!(reason(&d)["url"], "https://github.com/o/r/pull/1");

    // Unblock opens a new one; GitHub then reports a conflict.
    rig.post("/v1/tickets/DEMO-1/actions/unblock", json!({}))
        .await;
    assert_eq!(rig.creates(), 2, "a closed pull request is not reused");
    rig.gh_edit(|s| s["prs"][1]["mergeable"] = json!("CONFLICTING"));
    rig.tick().await;
    let d = rig.card("DEMO-1").await;
    assert_eq!(reason(&d)["kind"], "merge_conflict");
    assert!(
        reason(&d)["detail"]
            .as_str()
            .expect("detail")
            .contains("pull/2"),
        "{d}"
    );
}
