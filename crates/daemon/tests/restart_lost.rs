//! An update restarts the daemon and the agents' processes go with it. Through the full router
//! with real processes (the workspace's `fake-agent`, never a real agent): after the restart the
//! card is Running again with a new process and no person involved; lost again right after, it is
//! Blocked with both times in its reason, and unblocking it starts the agent again.

use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::http::{Method, Request};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tokio::sync::Notify;
use toknila_core::{AgentRole, TicketId};
use toknila_daemon::recovery::recover;
use toknila_daemon::runner::Runners;
use toknila_daemon::server::{AppState, router};
use toknila_daemon::shipyard::GitShipyard;
use toknila_daemon::store::Store;
use tower::ServiceExt;

const TOKEN: &str = "t";
const AGENT: &str = env!("CARGO_BIN_EXE_fake-agent");
const WAIT: Duration = Duration::from_secs(30);

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .output()
        .expect("git runs");
    assert!(out.status.success(), "git {args:?}");
}

/// One daemon lifetime over the shared database: its store, its runners and its router. The
/// supervisor is never started, so a process that dies is recorded nowhere, as when a daemon is
/// killed.
struct Daemon {
    app: axum::Router,
    store: Store,
    runners: Arc<Runners>,
    _ended: tokio::sync::mpsc::UnboundedReceiver<toknila_app::runner::RunEnded>,
}

impl Daemon {
    fn start(root: &Path) -> Daemon {
        let profiles = root.join("p");
        std::fs::create_dir_all(&profiles).expect("profiles");
        std::fs::write(
            profiles.join("holder.toml"),
            format!("command = ['{AGENT}', 'print', 'holding', 'hold']\n"),
        )
        .expect("profile");
        let (runners, ended) = Runners::new(root.join("runs"), profiles.clone());
        let store = Store::open(&root.join("t.db"))
            .expect("store")
            .with_runner(runners.clone())
            .with_shipyard(Arc::new(GitShipyard::new(root.join("ships"))))
            .with_merger(Arc::new(toknila_daemon::github::Mergers::new()));
        let app = router(AppState {
            store: store.clone(),
            token: TOKEN.into(),
            shutdown: Arc::new(Notify::new()),
            profiles,
            runners: runners.clone(),
            tools: Arc::new(toknila_daemon::tools::SystemTools),
            usage: Arc::new(toknila_daemon::usage::Usage::disabled()),
            home: root.to_owned(),
            ships: root.join("ships"),
            trash: Arc::new(toknila_daemon::trash::NoTrash),
            pdf_renderer: Arc::new(toknila_daemon::designs::thumb::NoRenderer),
        });
        Daemon {
            app,
            store,
            runners,
            _ended: ended,
        }
    }

    async fn send(&self, method: Method, uri: &str, body: Value) -> Value {
        let req = Request::builder()
            .method(method)
            .uri(uri)
            .header("authorization", format!("Bearer {TOKEN}"))
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .expect("request");
        let resp = self.app.clone().oneshot(req).await.expect("response");
        let status = resp.status();
        let bytes = resp.into_body().collect().await.expect("body").to_bytes();
        let v: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        assert!(status.is_success(), "{uri}: {status} {v}");
        v
    }

    async fn card(&self) -> Value {
        self.send(Method::GET, "/v1/tickets/DEMO-1", Value::Null)
            .await
    }

    async fn id(&self) -> TicketId {
        self.card().await["ticket"]["id"]
            .as_str()
            .expect("id")
            .parse()
            .expect("ticket id")
    }

    /// Polls until `ok` holds of the card, with a deadline.
    async fn until(&self, what: &str, ok: impl Fn(&Value) -> bool) -> Value {
        let until = Instant::now() + WAIT;
        loop {
            let c = self.card().await;
            if ok(&c) {
                return c;
            }
            assert!(Instant::now() < until, "no {what}: {}", c["ticket"]);
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    /// Polls until the card's worker process is alive and says it holds.
    async fn holding(&self) {
        let id = self.id().await;
        let until = Instant::now() + WAIT;
        loop {
            let said = self
                .runners
                .read(id, AgentRole::Worker, None)
                .map(|c| String::from_utf8_lossy(&c.bytes).into_owned())
                .unwrap_or_default();
            if self.runners.live().contains(&id) && said.contains("holding") {
                return;
            }
            assert!(Instant::now() < until, "no live process: {said:?}");
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }
}

#[tokio::test]
async fn a_restart_brings_a_running_card_back_and_a_second_loss_right_after_blocks_it() {
    let tmp = tempfile::tempdir().expect("tmp");
    let repo = tmp.path().join("repo");
    std::fs::create_dir_all(&repo).expect("mkdir");
    git(&repo, &["init", "--quiet"]);
    git(&repo, &["symbolic-ref", "HEAD", "refs/heads/develop"]);
    std::fs::write(repo.join("README.md"), "hi\n").expect("write");
    git(&repo, &["add", "README.md"]);
    git(&repo, &["commit", "--quiet", "-m", "first"]);

    // The first daemon starts the card.
    let one = Daemon::start(tmp.path());
    one.send(
        Method::POST,
        "/v1/projects",
        json!({ "key": "DEMO", "repo_path": repo.to_string_lossy(), "default_profile": "holder" }),
    )
    .await;
    one.send(
        Method::POST,
        "/v1/projects/DEMO/chains",
        json!({ "name": "auth" }),
    )
    .await;
    one.send(
        Method::POST,
        "/v1/chains/auth/tickets",
        json!({ "title": "Card 1", "prompt": "Do it.", "profile": "holder" }),
    )
    .await;
    one.holding().await;
    assert_eq!(one.card().await["ticket"]["status"], "running");

    // The update: the daemon and its processes are gone, nothing was recorded.
    one.runners.stop_all();
    one.store.settled().await;
    drop(one);

    // The next daemon recovers: the card comes back by itself, with a process of its own.
    let two = Daemon::start(tmp.path());
    let report = recover(&two.store).await.expect("recover");
    assert_eq!(report.lost, ["DEMO-1"]);
    let back = two
        .until("a Running card without a pause", |c| {
            c["ticket"]["status"] == "running"
                && c["ticket"]["agent_paused"].is_null()
                && !c["ticket"]["lost_resume"].is_null()
        })
        .await;
    two.holding().await;
    let first = back["ticket"]["lost_resume"]
        .as_i64()
        .expect("time of the resume");

    // Lost again within ten minutes of that resume: a person decides, with both times named.
    two.runners.stop_all();
    two.store.settled().await;
    drop(two);
    let three = Daemon::start(tmp.path());
    recover(&three.store).await.expect("recover again");
    let blocked = three
        .until("a Blocked card", |c| c["ticket"]["status"] == "blocked")
        .await;
    let reason = &blocked["ticket"]["status_reason"]["reason"];
    assert_eq!(reason["kind"], "runner_lost");
    assert_eq!(reason["first"].as_i64(), Some(first), "{reason}");
    assert!(
        reason["second"].as_i64().is_some_and(|s| s >= first),
        "{reason}"
    );
    assert!(
        !three.runners.live().contains(&three.id().await),
        "no third start by itself"
    );

    // Unblock starts the agent again: never Running without a process behind it.
    three
        .send(
            Method::POST,
            "/v1/tickets/DEMO-1/actions/unblock",
            json!({}),
        )
        .await;
    three
        .until("a Running card with an agent", |c| {
            c["ticket"]["status"] == "running" && c["ticket"]["agent_paused"].is_null()
        })
        .await;
    three.holding().await;
    three.runners.stop_all();
    three.store.settled().await;
}
