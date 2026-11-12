//! `GET /agents` says what runs in each project: its assistant and the live runs of its cards,
//! through the full router with real runner processes (the workspace's `fake-agent`).

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Method, Request};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tokio::sync::Notify;
use toknila_app::runner::{RunRequest, Runner};
use toknila_core::{AgentRole, ProjectId, TicketId, planner};
use toknila_daemon::runner::Runners;
use toknila_daemon::server::{AppState, router};
use toknila_daemon::store::Store;
use tower::ServiceExt;

const TOKEN: &str = "t";
const AGENT: &str = env!("CARGO_BIN_EXE_fake-agent");

struct Rig {
    tmp: tempfile::TempDir,
    app: axum::Router,
    runners: Arc<Runners>,
}

impl Rig {
    fn new() -> Rig {
        let tmp = tempfile::tempdir().expect("tmp");
        let profiles = tmp.path().join("p");
        std::fs::create_dir_all(&profiles).expect("profiles");
        std::fs::write(
            profiles.join("fake.toml"),
            format!("command = ['{AGENT}', 'hold']\n"),
        )
        .expect("profile");
        let (runners, _) = Runners::new(tmp.path().join("runs"), profiles.clone());
        let store = Store::open(&tmp.path().join("t.db"))
            .expect("store")
            .with_runner(runners.clone());
        let app = router(AppState {
            store,
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
        Rig { tmp, app, runners }
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
        let status = resp.status();
        let bytes = resp.into_body().collect().await.expect("body").to_bytes();
        let v = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        assert!(status.is_success(), "{uri}: {status} {v}");
        v
    }

    async fn post(&self, uri: &str, body: Value) -> Value {
        self.call(Method::POST, uri, body).await
    }

    /// A project with one chain and cards by title; the simulated agents start each card.
    async fn project(&self, key: &str, chain: &str, titles: &[&str]) {
        self.post("/v1/projects", json!({ "key": key, "name": key }))
            .await;
        self.post(
            &format!("/v1/projects/{key}/chains"),
            json!({ "name": chain }),
        )
        .await;
        for t in titles {
            self.post(
                &format!("/v1/chains/{chain}/tickets"),
                json!({ "title": t }),
            )
            .await;
        }
    }

    async fn id_of(&self, uri: &str) -> String {
        let v = self.call(Method::GET, uri, Value::Null).await;
        v["id"]
            .as_str()
            .or_else(|| v["ticket"]["id"].as_str())
            .expect("id")
            .to_owned()
    }

    fn run(&self, ticket: TicketId, role: AgentRole, key: &str) {
        self.runners
            .start(&RunRequest {
                ticket,
                key: key.parse().expect("key"),
                role,
                profile: "fake".to_owned(),
                dir: self.tmp.path().display().to_string(),
                brief: Default::default(),
            })
            .expect("start");
    }

    async fn agents_of(&self, project: &str) -> Value {
        let id = self.id_of(&format!("/v1/projects/{project}")).await;
        let all = self.call(Method::GET, "/v1/agents", Value::Null).await;
        all.as_array()
            .expect("array")
            .iter()
            .find(|a| a["project"] == id)
            .expect("the project is listed")
            .clone()
    }
}

/// `(role, card key or null)` of each run, in order.
fn runs(agents: &Value) -> Vec<(String, Value)> {
    agents["runs"]
        .as_array()
        .expect("runs")
        .iter()
        .map(|r| {
            assert!(r["since"].as_i64().expect("since") > 0, "{r}");
            (
                r["role"].as_str().expect("role").to_owned(),
                r["card"]["key"].clone(),
            )
        })
        .collect()
}

#[tokio::test]
async fn each_project_lists_its_assistant_and_its_own_cards_only() {
    let rig = Rig::new();
    rig.project("AAA", "ca", &["Alpha"]).await;
    rig.project("BBB", "cb", &["Beta"]).await;
    let pa: ProjectId = rig
        .id_of("/v1/projects/AAA")
        .await
        .parse()
        .expect("project id");
    let a1: TicketId = rig.id_of("/v1/tickets/AAA-1").await.parse().expect("id");
    let b1: TicketId = rig.id_of("/v1/tickets/BBB-1").await.parse().expect("id");
    rig.run(planner::run_key(pa), AgentRole::Planner, "AAA-1");
    rig.run(a1, AgentRole::Worker, "AAA-1");
    rig.run(b1, AgentRole::Worker, "BBB-1");

    let a = rig.agents_of("AAA").await;
    assert_eq!(
        runs(&a),
        [
            ("planner".to_owned(), Value::Null),
            ("worker".to_owned(), json!("AAA-1"))
        ]
    );
    assert_eq!(a["runs"][1]["card"]["title"], "Alpha");
    assert_eq!(a["used"], 1, "the assistant holds no slot: {a}");
    let b = rig.agents_of("BBB").await;
    assert_eq!(runs(&b), [("worker".to_owned(), json!("BBB-1"))]);

    rig.runners.stop_all();
}

#[tokio::test]
async fn a_reviewers_run_is_listed_with_its_role_while_it_runs() {
    let rig = Rig::new();
    rig.project("AAA", "ca", &["Alpha"]).await;
    rig.call(
        Method::PATCH,
        "/v1/projects/AAA",
        json!({ "review_mode": "human" }),
    )
    .await;
    rig.post(
        "/v1/tickets/AAA-1/events",
        json!({ "event": "finished", "summary": toknila_core::handin::from_plain("done") }),
    )
    .await;
    let t = rig
        .call(Method::GET, "/v1/tickets/AAA-1", Value::Null)
        .await;
    assert_eq!(t["ticket"]["status"], "review");
    assert!(runs(&rig.agents_of("AAA").await).is_empty());

    let a1: TicketId = rig.id_of("/v1/tickets/AAA-1").await.parse().expect("id");
    rig.run(a1, AgentRole::Reviewer, "AAA-1");
    assert_eq!(
        runs(&rig.agents_of("AAA").await),
        [("reviewer".to_owned(), json!("AAA-1"))]
    );

    rig.runners.stop_all();
}
