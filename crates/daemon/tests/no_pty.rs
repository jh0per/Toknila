//! A computer with no free pty, through the full router with a real git ship: the runner reports
//! `NoPty`, and the card is blocked with a sentence a person can act on, not as a crash.

use std::path::Path;
use std::process::Command;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Method, Request};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tokio::sync::Notify;
use toknila_app::runner::{NoRunner, RunRequest, Runner, RunnerError, RunnerState, Started};
use toknila_core::instructions::terminal::Size;
use toknila_core::{AgentRole, TicketId};
use toknila_daemon::server::{AppState, router};
use toknila_daemon::shipyard::GitShipyard;
use toknila_daemon::store::Store;
use tower::ServiceExt;

const TOKEN: &str = "t";

/// A host whose every start finds no pty; the rest is the runner of a host that has none.
#[derive(Debug)]
struct NoPtyRunner;

impl Runner for NoPtyRunner {
    fn start(&self, _: &RunRequest) -> Result<Started, RunnerError> {
        Err(RunnerError::NoPty("Unknown error: -6".to_owned()))
    }
    fn stop(&self, t: TicketId) {
        NoRunner.stop(t);
    }
    fn stop_role(&self, t: TicketId, r: AgentRole) {
        NoRunner.stop_role(t, r);
    }
    fn live_runners(&self) -> Vec<(TicketId, AgentRole)> {
        Vec::new()
    }
    fn write(&self, t: TicketId, r: AgentRole, b: &[u8]) -> Result<(), RunnerError> {
        NoRunner.write(t, r, b)
    }
    fn state(&self, _: TicketId) -> Option<RunnerState> {
        None
    }
    fn resize(&self, t: TicketId, s: Size) -> Result<(), RunnerError> {
        NoRunner.resize(t, s)
    }
    fn resize_role(&self, t: TicketId, r: AgentRole, s: Size) -> Result<(), RunnerError> {
        NoRunner.resize_role(t, r, s)
    }
}

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

#[tokio::test]
async fn a_start_that_finds_no_pty_blocks_the_card_with_a_sentence_and_not_as_a_crash() {
    let tmp = tempfile::tempdir().expect("tmp");
    let repo = tmp.path().join("repo");
    std::fs::create_dir_all(&repo).expect("mkdir");
    git(&repo, &["init", "--quiet"]);
    git(&repo, &["symbolic-ref", "HEAD", "refs/heads/develop"]);
    std::fs::write(repo.join("README.md"), "hi\n").expect("write");
    git(&repo, &["add", "README.md"]);
    git(&repo, &["commit", "--quiet", "-m", "first"]);

    let profiles = tmp.path().join("p");
    std::fs::create_dir_all(&profiles).expect("profiles");
    std::fs::write(profiles.join("fake.toml"), "command = ['true']\n").expect("profile");
    let (runners, _ended) =
        toknila_daemon::runner::Runners::new(tmp.path().join("runs"), profiles.clone());
    let store = Store::open(&tmp.path().join("t.db"))
        .expect("store")
        .with_shipyard(Arc::new(GitShipyard::new(tmp.path().join("ships"))))
        .with_merger(Arc::new(toknila_daemon::github::Mergers::new()))
        .with_runner(Arc::new(NoPtyRunner));
    let app = router(AppState {
        store: store.clone(),
        token: TOKEN.into(),
        shutdown: Arc::new(Notify::new()),
        profiles,
        runners,
        tools: Arc::new(toknila_daemon::tools::SystemTools),
        usage: Arc::new(toknila_daemon::usage::Usage::disabled()),
        home: tmp.path().to_owned(),
        ships: tmp.path().join("ships"),
        trash: Arc::new(toknila_daemon::trash::NoTrash),
        pdf_renderer: Arc::new(toknila_daemon::designs::thumb::NoRenderer),
    });
    let call = |method: Method, uri: &'static str, body: Value| {
        let app = app.clone();
        let store = store.clone();
        async move {
            let req = Request::builder()
                .method(method)
                .uri(uri)
                .header("authorization", format!("Bearer {TOKEN}"))
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .expect("request");
            let resp = app.oneshot(req).await.expect("response");
            store.settled().await;
            let status = resp.status();
            let bytes = resp.into_body().collect().await.expect("body").to_bytes();
            let v: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
            assert!(status.is_success(), "{uri}: {status} {v}");
            v
        }
    };
    call(
        Method::POST,
        "/v1/projects",
        json!({ "key": "DEMO", "repo_path": repo.to_string_lossy(), "default_profile": "fake" }),
    )
    .await;
    call(
        Method::POST,
        "/v1/projects/DEMO/chains",
        json!({ "name": "auth" }),
    )
    .await;
    call(
        Method::POST,
        "/v1/chains/auth/tickets",
        json!({ "title": "Add login", "prompt": "Let people in." }),
    )
    .await;
    let card = call(Method::GET, "/v1/tickets/DEMO-1", Value::Null).await;
    assert_eq!(card["ticket"]["status"], "blocked", "{card}");
    let reason = &card["ticket"]["status_reason"]["reason"];
    assert_eq!(reason["kind"], "manual", "not a crash: {reason}");
    assert_eq!(
        reason["text"],
        "No free terminal on this computer (every pty is in use). Close some terminals or \
         agents and press Unblock. (Unknown error: -6)"
    );
}
