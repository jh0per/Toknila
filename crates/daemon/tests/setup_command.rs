//! The project's setup command, through the full router with a real git ship, the real shell and
//! the fake agent: it runs once in a new ship before the agent starts, a failure blocks the card
//! without starting the agent, and a restarted run in the same ship does not run it again.

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

struct Rig {
    _tmp: tempfile::TempDir,
    app: axum::Router,
    store: Store,
    runners: Arc<toknila_daemon::runner::Runners>,
    _ended: tokio::sync::mpsc::UnboundedReceiver<toknila_app::runner::RunEnded>,
    ticket: Option<toknila_core::TicketId>,
}

impl Rig {
    /// A project whose agent is the fake one (it prints its kickoff and holds) and whose setup
    /// command is `setup`; the card DEMO-1 is created, so it starts.
    async fn new(setup: &str) -> Rig {
        let tmp = tempfile::tempdir().expect("tmp");
        let repo = tmp.path().join("repo");
        std::fs::create_dir_all(&repo).expect("mkdir");
        git(&repo, &["init", "--quiet"]);
        git(&repo, &["symbolic-ref", "HEAD", "refs/heads/develop"]);
        std::fs::write(repo.join("README.md"), "hi\n").expect("write");
        git(&repo, &["add", "README.md"]);
        git(&repo, &["commit", "--quiet", "-m", "first"]);

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
            ticket: None,
        };
        let project = rig
            .call(
                Method::POST,
                "/v1/projects",
                json!({ "key": "DEMO", "repo_path": repo.to_string_lossy(),
                        "default_profile": "fake", "setup_command": setup }),
            )
            .await;
        assert_eq!(project["setup_command"], setup);
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
                json!({ "title": "Add login", "prompt": "Let people in." }),
            )
            .await;
        rig.ticket = card["ticket"]["id"].as_str().and_then(|id| id.parse().ok());
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
        // The effects of the command (git, the setup, the agent) run after the answer.
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

    fn ship(card: &Value) -> PathBuf {
        PathBuf::from(card["ship"]["path"].as_str().expect("the card has a ship"))
    }

    /// What the worker printed so far; `None` when no agent ever started.
    fn output(&self) -> Option<String> {
        self.runners
            .read(self.ticket.expect("ticket"), AgentRole::Worker, None)
            .map(|c| String::from_utf8_lossy(&c.bytes).into_owned())
    }

    /// Waits for the agent's first line (its kickoff) and returns the output.
    async fn agent_started(&self) -> String {
        let until = Instant::now() + Duration::from_secs(20);
        loop {
            if let Some(out) = self.output().filter(|o| o.contains("DEMO-1")) {
                return out;
            }
            assert!(Instant::now() < until, "the agent never printed");
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }

    fn stop_agent(&self) {
        self.runners
            .stop_role(self.ticket.expect("ticket"), AgentRole::Worker);
    }
}

/// The notices by Toknila on the card, as `gist: what`.
fn notices(card: &Value) -> Vec<String> {
    card["comments"]
        .as_array()
        .expect("comments")
        .iter()
        .filter(|c| c["author"] == "system")
        .map(|c| format!("{}: {}", c["body"]["gist"], c["body"]["what"]))
        .collect()
}

#[tokio::test]
async fn the_setup_runs_in_the_ship_before_the_agent_and_is_noted_on_the_card() {
    let rig = Rig::new("echo ran> setup-ran").await;
    let out = rig.agent_started().await;
    let card = rig.card().await;
    let ship = Rig::ship(&card);
    // The agent's first line is out, so the setup was done before it started.
    assert!(ship.join("setup-ran").exists(), "{ship:?}");
    assert!(
        out.contains("already ran in your workspace"),
        "the kickoff says so: {out:?}"
    );
    let notes = notices(&card);
    assert!(
        notes
            .iter()
            .any(|n| n.contains("Setup ran: echo ran> setup-ran")
                && n.contains("exited with code 0")),
        "{notes:?}"
    );
    rig.stop_agent();
}

#[tokio::test]
async fn a_failing_setup_blocks_the_card_without_an_agent_and_unblock_runs_it_again() {
    let rig = Rig::new("echo boom-line && exit 1").await;
    let card = rig.card().await;
    assert_eq!(card["ticket"]["status"], "blocked", "{card}");
    let reason = &card["ticket"]["status_reason"]["reason"];
    assert_eq!(reason["kind"], "setup_failed", "{reason}");
    assert_eq!(reason["command"], "echo boom-line && exit 1");
    assert!(
        reason["output"]
            .as_str()
            .unwrap_or("")
            .contains("boom-line"),
        "{reason}"
    );
    assert!(rig.output().is_none(), "no agent process was started");
    assert!(
        notices(&card)
            .iter()
            .any(|n| n.contains("Setup failed: echo boom-line")),
        "{:?}",
        notices(&card)
    );

    // A person fixes the command; unblocking runs the setup again, and then the agent starts.
    rig.call(
        Method::PATCH,
        "/v1/projects/DEMO",
        json!({ "setup_command": "echo fixed> setup-ran" }),
    )
    .await;
    rig.call(
        Method::POST,
        "/v1/tickets/DEMO-1/actions/unblock",
        json!({}),
    )
    .await;
    rig.agent_started().await;
    let card = rig.card().await;
    assert_eq!(card["ticket"]["status"], "running", "{card}");
    assert!(Rig::ship(&card).join("setup-ran").exists());
    rig.stop_agent();
}

#[tokio::test]
async fn a_restarted_run_in_the_same_ship_does_not_run_the_setup_again() {
    let rig = Rig::new("echo x>> count.txt").await;
    rig.agent_started().await;
    let ship = Rig::ship(&rig.card().await);
    let count = || {
        std::fs::read_to_string(ship.join("count.txt"))
            .expect("count")
            .lines()
            .count()
    };
    assert_eq!(count(), 1);

    // The run ends and starts again in the same ship (a person blocks the card, then unblocks).
    rig.stop_agent();
    rig.call(
        Method::POST,
        "/v1/tickets/DEMO-1/actions/block",
        json!({ "text": "pause" }),
    )
    .await;
    rig.call(
        Method::POST,
        "/v1/tickets/DEMO-1/actions/unblock",
        json!({}),
    )
    .await;
    rig.agent_started().await;
    assert_eq!(count(), 1, "the setup ran a second time");
    rig.stop_agent();
}
