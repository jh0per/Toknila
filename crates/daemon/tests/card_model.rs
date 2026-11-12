//! A card's own model reaches its agent: `--model opus` on a card that names it while the
//! project's worker model is `sonnet`, and the project's model on a card that does not. The agent
//! is the workspace's `fake-agent` under the name `claude`, which prints the arguments it got.

use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::http::{Method, Request};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tokio::sync::Notify;
use toknila_app::runner::Runner;
use toknila_core::{AgentRole, TicketId};
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
}

impl Rig {
    async fn new() -> Rig {
        let tmp = tempfile::tempdir().expect("tmp");
        let repo = tmp.path().join("repo");
        std::fs::create_dir_all(&repo).expect("mkdir");
        git(&repo, &["init", "--quiet"]);
        git(&repo, &["symbolic-ref", "HEAD", "refs/heads/develop"]);
        std::fs::write(repo.join("README.md"), "hi\n").expect("write");
        git(&repo, &["add", "README.md"]);
        git(&repo, &["commit", "--quiet", "-m", "first"]);

        // Only a program named `claude` gets `--model`: the fake agent, copied under that name.
        let bin = tmp.path().join("bin");
        std::fs::create_dir_all(&bin).expect("bin");
        let claude = bin.join(format!("claude{}", std::env::consts::EXE_SUFFIX));
        std::fs::copy(env!("CARGO_BIN_EXE_fake-agent"), &claude).expect("copy the agent");
        let profiles = tmp.path().join("p");
        std::fs::create_dir_all(&profiles).expect("profiles");
        std::fs::write(
            profiles.join("fake.toml"),
            format!(
                "command = ['{}', 'print', '{{kickoff}}', 'argv', 'hold']\n",
                claude.display()
            ),
        )
        .expect("profile");
        let (runners, ended) =
            toknila_daemon::runner::Runners::new(tmp.path().join("runs"), profiles.clone());
        let store = Store::open(&tmp.path().join("t.db"))
            .expect("store")
            .with_shipyard(Arc::new(GitShipyard::new(tmp.path().join("ships"))))
            .with_merger(Arc::new(toknila_daemon::github::Mergers::new()))
            .with_runner(runners.clone());
        let app = router(AppState {
            store: store.clone(),
            token: TOKEN.into(),
            shutdown: Arc::new(Notify::new()),
            profiles,
            runners: runners.clone(),
            tools: Arc::new(toknila_daemon::tools::SystemTools),
            usage: Arc::new(toknila_daemon::usage::Usage::disabled()),
            home: tmp.path().to_owned(),
            ships: tmp.path().join("ships"),
            trash: Arc::new(toknila_daemon::trash::NoTrash),
            pdf_renderer: Arc::new(toknila_daemon::designs::thumb::NoRenderer),
        });
        let rig = Rig {
            _tmp: tmp,
            app,
            store,
            runners,
            _ended: ended,
        };
        rig.call(
            Method::POST,
            "/v1/projects",
            json!({ "key": "DEMO", "repo_path": repo.to_string_lossy(),
                    "default_profile": "fake" }),
        )
        .await;
        rig.call(
            Method::PATCH,
            "/v1/projects/DEMO",
            json!({ "worker_model": "sonnet" }),
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
        // The effects of the command (git, the agent) run after the answer.
        self.store.settled().await;
        let status = resp.status();
        let bytes = resp.into_body().collect().await.expect("body").to_bytes();
        let v = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        assert!(status.is_success(), "{uri}: {status} {v}");
        v
    }

    /// A chain with one card, which starts at once; the arguments its agent printed.
    async fn argv_of_card(&self, chain: &str, card: Value) -> String {
        self.call(
            Method::POST,
            "/v1/projects/DEMO/chains",
            json!({ "name": chain }),
        )
        .await;
        let made = self
            .call(Method::POST, &format!("/v1/chains/{chain}/tickets"), card)
            .await;
        let id: TicketId = made["ticket"]["id"]
            .as_str()
            .and_then(|id| id.parse().ok())
            .expect("ticket id");
        let until = Instant::now() + Duration::from_secs(20);
        let argv = loop {
            let out = self
                .runners
                .read(id, AgentRole::Worker, None)
                .map(|c| String::from_utf8_lossy(&c.bytes).into_owned());
            if let Some(line) = out
                .as_deref()
                .and_then(|o| o.lines().find(|l| l.starts_with("argv=")))
            {
                break line.to_owned();
            }
            assert!(Instant::now() < until, "the agent never printed: {out:?}");
            tokio::time::sleep(Duration::from_millis(25)).await;
        };
        self.runners.stop_role(id, AgentRole::Worker);
        argv
    }
}

#[tokio::test]
async fn a_card_runs_on_its_own_model_and_one_without_takes_the_projects() {
    let rig = Rig::new().await;
    let own = rig
        .argv_of_card("deep", json!({ "title": "Design it", "model": "opus" }))
        .await;
    assert!(own.contains("--model opus"), "{own}");
    assert!(!own.contains("sonnet"), "{own}");
    let plain = rig
        .argv_of_card("routine", json!({ "title": "Do it" }))
        .await;
    assert!(plain.contains("--model sonnet"), "{plain}");
    assert!(!plain.contains("opus"), "{plain}");
}
