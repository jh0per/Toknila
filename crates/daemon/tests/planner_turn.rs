//! The planner's turn through the full router, with a live planner run (the workspace's
//! `fake-agent`, holding): `POST /v1/planner/activity` with the run's token starts the turn,
//! names its step and ends it; `GET /v1/projects/{project}/planner` answers it; only the start
//! and the end are journal entries (`planner.turn`).

use std::path::PathBuf;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tokio::sync::Notify;
use toknila_app::runner::{RunRequest, Runner};
use toknila_core::{AgentRole, ProjectId, RunId, TicketId, TicketKey, planner};
use toknila_daemon::runner::{Grant, Runners};
use toknila_daemon::server::{AppState, now, router};
use toknila_daemon::store::Store;
use tower::ServiceExt;

const TOKEN: &str = "t";
const AGENT: &str = env!("CARGO_BIN_EXE_fake-agent");
const ACTIVITY: &str = "/v1/planner/activity";

struct Rig {
    _tmp: tempfile::TempDir,
    app: axum::Router,
    runners: Arc<Runners>,
    /// The folder the planner works in.
    repo: PathBuf,
}

impl Drop for Rig {
    fn drop(&mut self) {
        self.runners.stop_all();
    }
}

impl Rig {
    fn new() -> Rig {
        let tmp = tempfile::tempdir().expect("tmp");
        let profiles = tmp.path().join("p");
        std::fs::create_dir_all(&profiles).expect("profiles");
        std::fs::write(
            profiles.join("fake.toml"),
            format!("command = ['{AGENT}', 'hold']\nrole = \"planner\"\n"),
        )
        .expect("profile");
        let repo = tmp.path().join("repo");
        std::fs::create_dir_all(&repo).expect("repo");
        let (runners, _) = Runners::new(tmp.path().join("runs"), profiles.clone());
        let store = Store::open(&tmp.path().join("t.db")).expect("store");
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
        Rig {
            _tmp: tmp,
            app,
            runners,
            repo,
        }
    }

    async fn call(
        &self,
        token: &str,
        method: Method,
        uri: &str,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let mut req = Request::builder()
            .method(method)
            .uri(uri)
            .header("authorization", format!("Bearer {token}"));
        let body = match body {
            Some(v) => {
                req = req.header("content-type", "application/json");
                Body::from(v.to_string())
            }
            None => Body::empty(),
        };
        let resp = self
            .app
            .clone()
            .oneshot(req.body(body).expect("request"))
            .await
            .expect("response");
        let status = resp.status();
        let bytes = resp.into_body().collect().await.expect("body").to_bytes();
        let v = serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into()));
        (status, v)
    }

    async fn ok(&self, method: Method, uri: &str, body: Option<Value>) -> Value {
        let (status, v) = self.call(TOKEN, method, uri, body).await;
        assert!(status.is_success(), "{uri}: {status} {v}");
        v
    }

    /// Project DEMO, and its id.
    async fn project(&self) -> ProjectId {
        self.ok(
            Method::POST,
            "/v1/projects",
            Some(json!({ "key": "DEMO", "name": "Demo" })),
        )
        .await;
        self.ok(Method::GET, "/v1/projects/DEMO", None).await["id"]
            .as_str()
            .and_then(|s| s.parse().ok())
            .expect("project id")
    }

    /// Project DEMO with its planner running in `repo`; the run's token and the project's id.
    async fn running_planner(&self) -> (String, ProjectId) {
        let project = self.project().await;
        let key = planner::run_key(project);
        let started = self
            .runners
            .start(&RunRequest {
                ticket: key,
                key: TicketKey::planner("DEMO".parse().expect("key")),
                role: AgentRole::Planner,
                profile: "fake".to_owned(),
                dir: self.repo.display().to_string(),
                brief: Default::default(),
            })
            .expect("start");
        let grant = Grant {
            ticket: key,
            run: started.run.id,
            role: AgentRole::Planner,
        };
        let token = self.runners.tokens().issue(grant, now()).expect("token");
        (token, project)
    }

    async fn signal(&self, token: &str, body: Value) -> (StatusCode, Value) {
        self.call(token, Method::POST, ACTIVITY, Some(body)).await
    }

    async fn state(&self) -> Value {
        self.ok(Method::GET, "/v1/projects/DEMO/planner", None)
            .await
    }

    async fn journal(&self) -> Vec<Value> {
        self.ok(Method::GET, "/v1/events?since=0", None)
            .await
            .as_array()
            .expect("events")
            .clone()
    }

    /// The payloads of the `planner.turn` entries, oldest first.
    async fn turns(&self) -> Vec<Value> {
        self.journal()
            .await
            .into_iter()
            .filter(|e| e["entity"] == "planner")
            .map(|e| {
                assert_eq!(e["kind"], "turn", "{e}");
                e["payload"].clone()
            })
            .collect()
    }
}

/// `(working, step, since)` of the planner's state.
fn turn_of(state: &Value) -> (Value, Value, Value) {
    (
        state["working"].clone(),
        state["step"].clone(),
        state["since"].clone(),
    )
}

#[tokio::test]
async fn a_message_starts_the_turn_with_its_start_time_and_journals_it_once() {
    let rig = Rig::new();
    let (token, project) = rig.running_planner().await;
    let idle = rig.state().await;
    assert_eq!(idle["running"], true, "{idle}");
    assert_eq!(turn_of(&idle), (json!(false), json!(null), json!(null)));

    let before = now().0;
    let (status, body) = rig
        .signal(&token, json!({ "signal": "user_prompt_submit" }))
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    let on = rig.state().await;
    assert_eq!(on["working"], true, "{on}");
    assert_eq!(on["step"], "Reading your message");
    let since = on["since"].as_i64().expect("since is set");
    assert!(
        since >= before && since <= now().0,
        "{since} not in [{before}, now]"
    );
    assert_eq!(
        rig.turns().await,
        [json!({ "project": project.to_string(), "working": true })]
    );
}

#[tokio::test]
async fn a_tool_about_to_run_names_the_step_and_writes_no_journal_entry() {
    let rig = Rig::new();
    let (token, _) = rig.running_planner().await;
    rig.signal(&token, json!({ "signal": "user_prompt_submit" }))
        .await;
    let since = rig.state().await["since"].clone();
    let entries = rig.journal().await.len();

    let file = rig
        .repo
        .join("apps")
        .join("web")
        .join("src")
        .join("views")
        .join("ChatPane.tsx");
    let (status, body) = rig
        .signal(
            &token,
            json!({ "signal": "pre_tool_use", "tool_name": "Read",
                    "tool": format!("Read: {}", file.display()) }),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    let state = rig.state().await;
    assert_eq!(
        turn_of(&state),
        (
            json!(true),
            json!("Reading apps/web/src/views/ChatPane.tsx"),
            since.clone()
        ),
        "the path is the repository's, the turn keeps its start"
    );

    rig.signal(
        &token,
        json!({ "signal": "pre_tool_use", "tool_name": "mcp__toknila__add_card",
                "tool": "mcp__toknila__add_card" }),
    )
    .await;
    assert_eq!(rig.state().await["step"], "Adding a story");
    assert_eq!(
        rig.journal().await.len(),
        entries,
        "a step is no journal entry"
    );
}

#[tokio::test]
async fn stop_ends_the_turn_and_writes_the_second_entry() {
    let rig = Rig::new();
    let (token, project) = rig.running_planner().await;
    rig.signal(&token, json!({ "signal": "user_prompt_submit" }))
        .await;
    rig.signal(
        &token,
        json!({ "signal": "pre_tool_use", "tool_name": "mcp__toknila__say" }),
    )
    .await;
    assert_eq!(rig.state().await["step"], "Writing to you");

    let (status, body) = rig.signal(&token, json!({ "signal": "stop" })).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    let off = rig.state().await;
    assert_eq!(off["running"], true, "the planner still runs: {off}");
    assert_eq!(turn_of(&off), (json!(false), json!(null), json!(null)));
    let id = project.to_string();
    let both = [
        json!({ "project": id, "working": true }),
        json!({ "project": id, "working": false }),
    ];
    assert_eq!(rig.turns().await, both);

    // A stop outside a turn ends nothing and journals nothing.
    rig.signal(&token, json!({ "signal": "stop" })).await;
    assert_eq!(rig.turns().await, both);
}

#[tokio::test]
async fn stopping_the_planner_clears_a_turn_that_was_on() {
    let rig = Rig::new();
    let (token, project) = rig.running_planner().await;
    rig.signal(&token, json!({ "signal": "user_prompt_submit" }))
        .await;
    assert_eq!(rig.state().await["working"], true);

    let stopped = rig
        .ok(
            Method::POST,
            "/v1/projects/DEMO/planner/stop",
            Some(json!({})),
        )
        .await;
    assert_eq!(stopped["running"], false, "{stopped}");
    assert_eq!(turn_of(&stopped), (json!(false), json!(null), json!(null)));
    assert_eq!(
        turn_of(&rig.state().await),
        (json!(false), json!(null), json!(null))
    );
    let id = project.to_string();
    let both = [
        json!({ "project": id, "working": true }),
        json!({ "project": id, "working": false }),
    ];
    assert_eq!(
        rig.turns().await,
        both,
        "the stop ends the turn in the journal"
    );

    // A hook of the ending run (its token may still be live, or revoked already) starts nothing.
    let (status, _) = rig
        .signal(&token, json!({ "signal": "user_prompt_submit" }))
        .await;
    assert!(
        [StatusCode::NO_CONTENT, StatusCode::UNAUTHORIZED].contains(&status),
        "{status}"
    );
    assert_eq!(rig.state().await["working"], false);
    assert_eq!(rig.turns().await, both);
}

#[tokio::test]
async fn a_planner_that_is_not_running_is_never_working() {
    let rig = Rig::new();
    let project = rig.project().await;
    // A planner token of a run that is not alive.
    let grant = Grant {
        ticket: planner::run_key(project),
        run: RunId::new(),
        role: AgentRole::Planner,
    };
    let token = rig.runners.tokens().issue(grant, now()).expect("token");
    let (status, body) = rig
        .signal(&token, json!({ "signal": "user_prompt_submit" }))
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    let state = rig.state().await;
    assert_eq!(state["running"], false, "{state}");
    assert_eq!(turn_of(&state), (json!(false), json!(null), json!(null)));
    assert!(rig.turns().await.is_empty());
}

#[tokio::test]
async fn only_the_planners_token_reports_and_a_body_it_does_not_know_is_refused() {
    let rig = Rig::new();
    let (token, _) = rig.running_planner().await;
    rig.ok(
        Method::POST,
        "/v1/projects/DEMO/chains",
        Some(json!({ "name": "plan", "autostart": false })),
    )
    .await;
    let card: TicketId = rig
        .ok(
            Method::POST,
            "/v1/chains/plan/tickets",
            Some(json!({ "title": "A" })),
        )
        .await["ticket"]["id"]
        .as_str()
        .and_then(|s| s.parse().ok())
        .expect("card id");
    let worker = Grant {
        ticket: card,
        run: RunId::new(),
        role: AgentRole::Worker,
    };
    let worker = rig.runners.tokens().issue(worker, now()).expect("token");
    let start = json!({ "signal": "user_prompt_submit" });

    let (status, body) = rig.signal(&worker, start.clone()).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "a card's agent: {body}");
    let (status, body) = rig.signal(TOKEN, start.clone()).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "a person: {body}");

    let (status, body) = rig
        .signal(&token, json!({ "signal": "stop", "extra": 1 }))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(
        body["error"].as_str().unwrap_or("").contains("extra"),
        "{body}"
    );
    let (status, body) = rig
        .signal(&token, json!({ "signal": "post_tool_use" }))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

    assert_eq!(rig.state().await["working"], false);
    assert!(rig.turns().await.is_empty(), "nothing refused is journaled");
    // The control: the planner's own token is heard.
    let (status, _) = rig.signal(&token, start).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(rig.state().await["working"], true);
}
