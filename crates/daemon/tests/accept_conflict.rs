//! A conflict met at Accept, through the full router with a real git ship and the fake agent: the
//! card goes back to its agent as a round (the agent's runner is started again and is shown the
//! Return text), the agent merges the base and hands in again, and the card merges without a
//! second Accept. The test plays the agent's git and its `finished`; the agent process is the
//! workspace's `fake-agent`, which prints the kickoff it was started with.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tokio::sync::Notify;
use toknila_app::runner::Runner;
use toknila_core::AgentRole;
use toknila_daemon::server::{AppState, router};
use toknila_daemon::shipyard::GitShipyard;
use toknila_daemon::store::Store;
use tower::ServiceExt;

const TOKEN: &str = "t";

fn git(dir: &Path, args: &[&str]) {
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
}

fn commit_file(dir: &Path, name: &str, content: &str) {
    std::fs::write(dir.join(name), content).expect("write");
    git(dir, &["add", name]);
    git(dir, &["commit", "--quiet", "-m", name]);
}

struct Rig {
    _tmp: tempfile::TempDir,
    app: axum::Router,
    store: Store,
    runners: Arc<toknila_daemon::runner::Runners>,
    _ended: tokio::sync::mpsc::UnboundedReceiver<toknila_app::runner::RunEnded>,
    repo: PathBuf,
    ticket: toknila_core::TicketId,
}

impl Rig {
    async fn new() -> Rig {
        let tmp = tempfile::tempdir().expect("tmp");
        let repo = tmp.path().join("repo");
        std::fs::create_dir_all(&repo).expect("mkdir");
        git(&repo, &["init", "--quiet"]);
        git(&repo, &["symbolic-ref", "HEAD", "refs/heads/develop"]);
        commit_file(&repo, "README.md", "hi\n");

        let (runners, ended) =
            toknila_daemon::runner::Runners::new(tmp.path().join("runs"), tmp.path().join("p"));
        std::fs::create_dir_all(tmp.path().join("p")).expect("profiles");
        let agent = env!("CARGO_BIN_EXE_fake-agent");
        std::fs::write(
            tmp.path().join("p").join("fake.toml"),
            format!("command = ['{agent}', 'print', '{{kickoff}}', 'hold']\n"),
        )
        .expect("profile");
        let store = Store::open(&tmp.path().join("t.db"))
            .expect("store")
            .with_shipyard(Arc::new(GitShipyard::new(tmp.path().join("ships"))))
            .with_merger(Arc::new(toknila_daemon::github::Mergers::new()))
            .with_runner(runners.clone());
        let app = router(AppState {
            store: store.clone(),
            token: TOKEN.into(),
            shutdown: Arc::new(Notify::new()),
            profiles: tmp.path().join("p"),
            runners: runners.clone(),
            tools: Arc::new(toknila_daemon::tools::SystemTools),
            usage: Arc::new(toknila_daemon::usage::Usage::disabled()),
            home: tmp.path().to_owned(),
            ships: tmp.path().join("ships"),
            trash: Arc::new(toknila_daemon::trash::NoTrash),
            pdf_renderer: Arc::new(toknila_daemon::designs::thumb::NoRenderer),
        });
        let mut rig = Rig {
            _tmp: tmp,
            app,
            store,
            runners,
            _ended: ended,
            repo: repo.clone(),
            ticket: toknila_core::TicketId::new(),
        };
        rig.call(
            Method::POST,
            "/v1/projects",
            json!({ "key": "DEMO", "repo_path": repo.to_string_lossy(),
                    "default_profile": "fake", "default_review_mode": "human" }),
        )
        .await;
        rig.call(
            Method::POST,
            "/v1/projects/DEMO/chains",
            json!({ "name": "auth" }),
        )
        .await;
        let card = rig
            .call(
                Method::POST,
                "/v1/chains/auth/tickets",
                json!({ "title": "Edit readme", "prompt": "Say hi." }),
            )
            .await;
        rig.ticket = card["ticket"]["id"]
            .as_str()
            .and_then(|id| id.parse().ok())
            .expect("ticket id");
        rig
    }

    async fn send(&self, method: Method, uri: &str, body: Value) -> (StatusCode, Value) {
        let req = Request::builder()
            .method(method)
            .uri(uri)
            .header("authorization", format!("Bearer {TOKEN}"))
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .expect("request");
        let resp = self.app.clone().oneshot(req).await.expect("response");
        // The effects of the command (git, the merge, the agent) run after the answer.
        self.store.settled().await;
        let status = resp.status();
        let bytes = resp.into_body().collect().await.expect("body").to_bytes();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }

    async fn call(&self, method: Method, uri: &str, body: Value) -> Value {
        let (status, v) = self.send(method, uri, body).await;
        assert!(status.is_success(), "{uri}: {status} {v}");
        v
    }

    async fn card(&self) -> Value {
        self.call(Method::GET, "/v1/tickets/DEMO-1", Value::Null)
            .await
    }

    /// The worker's output with line breaks removed, so a sentence is found wherever it wrapped.
    fn output(&self) -> String {
        self.runners
            .read(self.ticket, AgentRole::Worker, None)
            .map(|c| String::from_utf8_lossy(&c.bytes).into_owned())
            .unwrap_or_default()
            .replace(['\r', '\n'], "")
    }

    async fn output_with(&self, what: &str) -> String {
        let until = Instant::now() + Duration::from_secs(30);
        loop {
            let out = self.output();
            if out.contains(what) {
                return out;
            }
            assert!(
                Instant::now() < until,
                "the agent never printed {what:?}: {out:?}"
            );
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }

    async fn hand_in(&self, summary: &str) {
        self.call(
            Method::POST,
            "/v1/tickets/DEMO-1/events",
            json!({ "event": "finished", "summary": toknila_core::handin::from_plain(summary) }),
        )
        .await;
    }
}

#[tokio::test]
async fn a_conflict_at_accept_restarts_the_agent_with_the_return_and_its_next_hand_in_merges() {
    let rig = Rig::new().await;
    rig.output_with("DEMO-1").await;
    let ship = PathBuf::from(
        rig.card().await["ship"]["path"]
            .as_str()
            .expect("the card has a ship"),
    );
    // The agent works and hands in; the base has not moved, so the card waits for Accept.
    commit_file(&ship, "README.md", "ship says hi\n");
    rig.hand_in("done").await;
    assert_eq!(rig.card().await["ticket"]["status"], "review");
    rig.runners.stop_role(rig.ticket, AgentRole::Worker);

    // The base moves with a conflicting change, then the person accepts.
    commit_file(&rig.repo, "README.md", "base says hi\n");
    let base = Command::new("git")
        .arg("-C")
        .arg(&rig.repo)
        .args(["rev-parse", "develop"])
        .output()
        .expect("git")
        .stdout;
    rig.call(Method::POST, "/v1/tickets/DEMO-1/actions/accept", json!({}))
        .await;

    // The card is back with its agent, whose new run is shown the Return text.
    let card = rig.card().await;
    assert_eq!(card["ticket"]["status"], "running", "{card}");
    assert_ne!(card["ticket"]["status"], "blocked");
    let out = rig.output_with("This story was returned").await;
    for want in [
        "Conflicting files: README.md",
        "git merge develop",
        "run the gates",
    ] {
        assert!(out.contains(want), "{want:?} in the kickoff: {out}");
    }
    let after = Command::new("git")
        .arg("-C")
        .arg(&rig.repo)
        .args(["rev-parse", "develop"])
        .output()
        .expect("git")
        .stdout;
    assert_eq!(after, base, "develop is untouched by the conflict");

    // The agent merges the base, resolves it and hands in again: no second Accept is needed.
    git(&ship, &["merge", "--no-edit", "-s", "ours", "develop"]);
    rig.hand_in("merged").await;
    let card = rig.card().await;
    assert_eq!(card["ticket"]["status"], "merged", "{card}");
    let shown = Command::new("git")
        .arg("-C")
        .arg(&rig.repo)
        .args(["show", "develop:README.md"])
        .output()
        .expect("git");
    assert_eq!(String::from_utf8_lossy(&shown.stdout), "ship says hi\n");
    rig.runners.stop_role(rig.ticket, AgentRole::Worker);
}
