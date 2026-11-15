//! A planner's process reports its turn as its hooks would: the workspace's `fake-agent` runs as a
//! project's planner against a daemon listening on a real port, and its script plays one turn with
//! the `activity` command (the message taken, a file read, an answer, the turn's end). Each part
//! waits for a line the test types, so the test reads `GET /v1/projects/{project}/planner` at
//! every state: working, then the step, then not working.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::http::{Method, Request};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tokio::sync::Notify;
use toknila_app::runner::{RunRequest, Runner};
use toknila_core::{AgentRole, TicketId, TicketKey, planner};
use toknila_daemon::runner::Runners;
use toknila_daemon::server::{AppState, router};
use toknila_daemon::store::Store;
use tower::ServiceExt;

const TOKEN: &str = "t";
const AGENT: &str = env!("CARGO_BIN_EXE_fake-agent");
const WAIT: Duration = Duration::from_secs(30);
const ANSWER: &str = "It holds one file, src/main.rs.";

struct Rig {
    _tmp: tempfile::TempDir,
    app: axum::Router,
    runners: Arc<Runners>,
    repo: PathBuf,
}

impl Drop for Rig {
    fn drop(&mut self) {
        self.runners.stop_all();
    }
}

impl Rig {
    /// A daemon on a local port whose `fake` profile is a planner playing one scripted turn.
    async fn new() -> Rig {
        let tmp = tempfile::tempdir().expect("tmp");
        let repo = tmp.path().join("repo");
        std::fs::create_dir_all(repo.join("src")).expect("repo");
        let file = repo.join("src").join("main.rs");
        std::fs::write(&file, "fn main() {}\n").expect("file");
        let profiles = tmp.path().join("p");
        std::fs::create_dir_all(&profiles).expect("profiles");
        let say = json!({ "text": ANSWER }).to_string();
        let steps = [
            "activity",
            "user_prompt_submit",
            "echo",
            "activity",
            "pre_tool_use",
            "Read",
            &format!("Read: {}", file.display()),
            "echo",
            "post",
            "/planner/say",
            &say,
            "activity",
            "stop",
            "hold",
        ]
        .map(|s| format!("'{s}'"))
        .join(", ");
        std::fs::write(
            profiles.join("fake.toml"),
            format!("command = ['{AGENT}', {steps}]\nrole = \"planner\"\n"),
        )
        .expect("profile");
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
        // The planner's process reports to a real listener, as its hooks do.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr = listener.local_addr().expect("address");
        runners.set_daemon_url(format!("http://{addr}"));
        let served = app.clone();
        tokio::spawn(async move { axum::serve(listener, served).await });
        Rig {
            _tmp: tmp,
            app,
            runners,
            repo,
        }
    }

    async fn ok(&self, method: Method, uri: &str, body: Option<Value>) -> Value {
        let mut req = Request::builder()
            .method(method)
            .uri(uri)
            .header("authorization", format!("Bearer {TOKEN}"));
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
        let v = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        assert!(status.is_success(), "{uri}: {status} {v}");
        v
    }

    /// Project DEMO with its planner started in `repo`; the run's key.
    async fn start_planner(&self) -> TicketId {
        self.ok(
            Method::POST,
            "/v1/projects",
            Some(json!({ "key": "DEMO", "name": "Demo" })),
        )
        .await;
        let project = self.ok(Method::GET, "/v1/projects/DEMO", None).await["id"]
            .as_str()
            .and_then(|s| s.parse().ok())
            .expect("project id");
        let key = planner::run_key(project);
        self.runners
            .start(&RunRequest {
                ticket: key,
                key: TicketKey::planner("DEMO".parse().expect("key")),
                role: AgentRole::Planner,
                profile: "fake".to_owned(),
                dir: self.repo.display().to_string(),
                brief: Default::default(),
            })
            .expect("start");
        key
    }

    async fn state(&self) -> Value {
        self.ok(Method::GET, "/v1/projects/DEMO/planner", None)
            .await
    }

    /// What the planner's terminal has shown, for a failure's message.
    fn output(&self, key: TicketId) -> String {
        self.runners
            .read(key, AgentRole::Planner, None)
            .map(|c| String::from_utf8_lossy(&c.bytes).into_owned())
            .unwrap_or_default()
    }

    /// Polls the planner's state until `ok` holds of it.
    async fn until(&self, key: TicketId, what: &str, ok: impl Fn(&Value) -> bool) -> Value {
        let until = Instant::now() + WAIT;
        loop {
            let state = self.state().await;
            if ok(&state) {
                return state;
            }
            assert!(
                Instant::now() < until,
                "no {what}: {state}\n{}",
                self.output(key)
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    /// Lets the script go on past its next `echo`.
    fn go_on(&self, key: TicketId) {
        self.runners
            .write(key, AgentRole::Planner, b"go\r")
            .expect("type into the planner");
    }
}

#[tokio::test]
async fn a_scripted_turn_shows_working_then_its_step_then_not_working() {
    let rig = Rig::new().await;
    let key = rig.start_planner().await;

    let on = rig
        .until(key, "turn", |s| s["working"] == json!(true))
        .await;
    assert_eq!(on["running"], true, "{on}");
    assert_eq!(on["step"], "Reading your message", "{on}");
    let since = on["since"].clone();
    assert!(since.is_i64(), "{on}");

    rig.go_on(key);
    let step = rig
        .until(key, "step", |s| s["step"] != json!("Reading your message"))
        .await;
    assert_eq!(step["working"], true, "{step}");
    assert_eq!(step["step"], "Reading src/main.rs", "{step}");
    assert_eq!(step["since"], since, "the turn keeps its start");

    rig.go_on(key);
    let off = rig
        .until(key, "the turn's end", |s| s["working"] == json!(false))
        .await;
    assert_eq!(off["running"], true, "the planner still runs: {off}");
    assert_eq!(off["step"], Value::Null, "{off}");
    assert_eq!(off["since"], Value::Null, "{off}");
    // The answer of the turn came before its end.
    let page = rig
        .ok(Method::GET, "/v1/projects/DEMO/conversation", None)
        .await;
    let said: Vec<&str> = page["messages"]
        .as_array()
        .expect("messages")
        .iter()
        .filter_map(|m| m["text"].as_str())
        .collect();
    assert_eq!(said, [ANSWER], "{page}");
    let out = rig.output(key);
    assert_eq!(out.matches("post=204").count(), 3, "{out}");
}
