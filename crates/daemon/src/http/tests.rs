//! One integration test per endpoint, through the full router (auth included).

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tokio::sync::Notify;
use tower::ServiceExt;

use crate::server::{AppState, router};
use crate::store::Store;

mod agent_tokens;
mod card_designs;
mod chain_settings;
mod chain_version;
mod commands;
mod conversation;
mod design;
mod design_delete;
mod designs;
mod discussion;
mod hand_in_only;
mod inbox;
mod merges;
mod moves;
mod parallel;
mod pins;
mod planner;
mod planning;
mod plans;
mod prepare;
mod profiles;
mod questions;
mod releases;
mod removal;
mod resume;
mod reviews;
mod roadmap;
mod settings;
mod ship_guard;
mod ships;
mod smart_defaults;
mod strays;
mod threads;
mod unknown_fields;
mod usage;

const TOKEN: &str = "test-token";

struct Api {
    _dir: tempfile::TempDir,
    app: axum::Router,
    store: Store,
    runners: Arc<crate::runner::Runners>,
    /// What this daemon moved to the Trash: folders land in `trash` of the test's folder.
    trash: Arc<FakeTrash>,
    /// The png the fake pdf renderer draws; `None`: the machine has no renderer.
    pdf_page: crate::designs::thumb::Page,
}

/// A Trash that moves a folder into a folder of the test's own and remembers it.
#[derive(Debug)]
struct FakeTrash {
    bin: std::path::PathBuf,
    moved: std::sync::Mutex<Vec<std::path::PathBuf>>,
}

impl crate::trash::Trasher for FakeTrash {
    fn trash(&self, path: &std::path::Path) -> Result<(), String> {
        let name = path.file_name().ok_or("a folder without a name")?;
        std::fs::create_dir_all(&self.bin).map_err(|e| e.to_string())?;
        std::fs::rename(path, self.bin.join(name)).map_err(|e| e.to_string())?;
        self.moved
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(path.to_owned());
        Ok(())
    }
}

impl Api {
    fn new() -> Self {
        Self::with_yard(None)
    }

    /// Like [`Api::new`] with a shipyard.
    fn with_yard(yard: Option<crate::store::Yard>) -> Self {
        Self::build(yard, Arc::new(crate::tools::SystemTools))
    }

    /// Like [`Api::new`] with injected tools (`git`, `claude`, `gh`).
    fn with_tools(tools: Arc<dyn crate::tools::Tools>) -> Self {
        Self::build(None, tools)
    }

    fn build(yard: Option<crate::store::Yard>, tools: Arc<dyn crate::tools::Tools>) -> Self {
        Self::open(tempfile::tempdir().expect("tempdir"), yard, tools)
    }

    /// A daemon over the database in `dir`: a new one, or the same one after a restart.
    fn open(
        dir: tempfile::TempDir,
        yard: Option<crate::store::Yard>,
        tools: Arc<dyn crate::tools::Tools>,
    ) -> Self {
        let mut store = Store::open(&dir.path().join("t.db")).expect("store");
        if let Some(yard) = yard {
            store = store
                .with_shipyard(yard)
                .with_merger(Arc::new(crate::merger::GitMerger::new()));
        }
        let (runners, _) =
            crate::runner::Runners::new(dir.path().join("runs"), dir.path().join("profiles"));
        let trash = Arc::new(FakeTrash {
            bin: dir.path().join("trash"),
            moved: Default::default(),
        });
        let pdf_page = crate::designs::thumb::Page::default();
        let state = AppState {
            store: store.clone(),
            token: TOKEN.into(),
            shutdown: Arc::new(Notify::new()),
            profiles: dir.path().join("profiles"),
            runners: runners.clone(),
            tools,
            // Agent transcripts live in the test's own folder, never in the real home.
            usage: Arc::new(crate::usage::Usage::with_sources(
                dir.path(),
                vec![crate::usage::Source {
                    format: Arc::new(toknila_core::usage::ClaudeCode),
                    roots: vec![dir.path().join("claude").join("projects")],
                }],
            )),
            home: dir.path().to_owned(),
            ships: dir.path().join("ships"),
            trash: trash.clone(),
            pdf_renderer: Arc::new(crate::designs::thumb::SwitchRenderer(pdf_page.clone())),
        };
        Self {
            _dir: dir,
            app: router(state),
            store,
            runners,
            trash,
            pdf_page,
        }
    }

    /// Stops this daemon and starts another over the same database.
    fn restart(self) -> Self {
        let Self {
            _dir,
            app,
            store,
            runners,
            ..
        } = self;
        drop((app, store, runners));
        Self::open(_dir, None, Arc::new(crate::tools::SystemTools))
    }

    async fn call(&self, method: Method, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
        self.call_with(TOKEN, method, uri, body).await
    }

    /// Like [`Api::call`] with another bearer token.
    async fn call_with(
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
        // The request answered at once; the host work it queued (git, merges, checks) runs in
        // tasks of its own. A test's client waits for them before it looks at the result.
        self.store.settled().await;
        let status = resp.status();
        let bytes = resp.into_body().collect().await.expect("body").to_bytes();
        let value = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes)
                .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into()))
        };
        (status, value)
    }

    async fn get(&self, uri: &str) -> Value {
        let (s, v) = self.call(Method::GET, uri, None).await;
        assert_eq!(s, StatusCode::OK, "GET {uri}: {v}");
        v
    }

    /// POST for tests. A project that names no profile is created as a simulation project (the
    /// tests send events by hand); `call` posts the body as it is.
    async fn post(&self, uri: &str, mut body: Value) -> Value {
        if uri == "/v1/projects"
            && let Some(o) = body.as_object_mut()
            && !o.contains_key("default_profile")
        {
            o.insert("default_profile".into(), json!("sim"));
            o.insert("simulated".into(), json!(true));
        }
        let (s, v) = self.call(Method::POST, uri, Some(body)).await;
        assert!(s.is_success(), "POST {uri}: {s} {v}");
        v
    }

    async fn status(&self, ticket: &str) -> String {
        self.get(&format!("/v1/tickets/{ticket}")).await["ticket"]["status"]
            .as_str()
            .expect("status")
            .to_owned()
    }

    /// DEMO project and a `demo` chain with autostart.
    async fn demo(&self) {
        self.post("/v1/projects", json!({ "key": "DEMO", "name": "Demo" }))
            .await;
        self.post(
            "/v1/projects/DEMO/chains",
            json!({ "name": "demo", "goal": "gate" }),
        )
        .await;
    }
}

#[tokio::test]
async fn requests_without_token_are_rejected() {
    let api = Api::new();
    let req = Request::builder()
        .uri("/v1/projects")
        .body(Body::empty())
        .expect("request");
    let resp = api.app.clone().oneshot(req).await.expect("response");
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let bytes = resp.into_body().collect().await.expect("body").to_bytes();
    // The same error shape as every other failure.
    assert_eq!(
        std::str::from_utf8(&bytes).expect("utf-8"),
        r#"{"error":"missing or wrong bearer token","code":"unauthorized"}"#
    );
}

#[tokio::test]
async fn projects_create_list_get() {
    let api = Api::new();
    let (s, p) = api
        .call(Method::POST, "/v1/projects", Some(json!({ "key": "DEMO" })))
        .await;
    assert_eq!(s, StatusCode::CREATED);
    assert_eq!(p["work_branch"], "develop");
    assert_eq!(p["default_profile"], "claude-code");
    assert!(p.get("simulated").is_none(), "{p}");
    assert_eq!(
        api.get("/v1/projects").await.as_array().map(Vec::len),
        Some(1)
    );
    assert_eq!(api.get("/v1/projects/DEMO").await["id"], p["id"]);
    assert_eq!(
        api.get(&format!("/v1/projects/{}", p["id"].as_str().expect("id")))
            .await["key"],
        "DEMO"
    );

    let (s, e) = api
        .call(Method::POST, "/v1/projects", Some(json!({ "key": "DEMO" })))
        .await;
    assert_eq!(
        (s, e["code"].as_str()),
        (StatusCode::CONFLICT, Some("conflict"))
    );
    let (s, _) = api
        .call(
            Method::POST,
            "/v1/projects",
            Some(json!({ "key": "demo!" })),
        )
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let (s, e) = api.call(Method::GET, "/v1/projects/NOPE", None).await;
    assert_eq!(
        (s, e["code"].as_str()),
        (StatusCode::NOT_FOUND, Some("not_found"))
    );
}

#[tokio::test]
async fn chains_create_list_get() {
    let api = Api::new();
    api.post("/v1/projects", json!({ "key": "DEMO" })).await;
    let (s, c) = api
        .call(
            Method::POST,
            "/v1/projects/DEMO/chains",
            Some(json!({ "name": "phase-0" })),
        )
        .await;
    assert_eq!(s, StatusCode::CREATED);
    assert_eq!(c["chain"]["autostart"], true);
    assert_eq!(c["summary"]["status"], "empty");
    let list = api.get("/v1/chains?project=DEMO").await;
    assert_eq!(list[0]["chain"]["name"], "phase-0");
    assert_eq!(
        api.get("/v1/chains/phase-0").await["chain"]["id"],
        c["chain"]["id"]
    );
    let (s, _) = api
        .call(
            Method::POST,
            "/v1/projects/DEMO/chains",
            Some(json!({ "name": "phase-0" })),
        )
        .await;
    assert_eq!(s, StatusCode::CONFLICT);
}

#[tokio::test]
async fn tickets_create_autostart_list_get_patch() {
    let api = Api::new();
    api.demo().await;
    let (s, a) = api
        .call(
            Method::POST,
            "/v1/chains/demo/tickets",
            Some(json!({ "title": "A", "subtasks": ["one", "two"], "acceptance": ["green"] })),
        )
        .await;
    assert_eq!(s, StatusCode::CREATED, "{a}");
    assert_eq!(a["ticket"]["key"], "DEMO-1");
    assert_eq!(a["ticket"]["status"], "running", "autostart");
    assert_eq!(a["subtasks"].as_array().map(Vec::len), Some(2));

    let b = api
        .post(
            "/v1/chains/demo/tickets",
            json!({ "title": "B", "after": ["DEMO-1"] }),
        )
        .await;
    assert_eq!(b["ticket"]["status"], "backlog");

    let all = api.get("/v1/tickets?chain=demo").await;
    assert_eq!(all.as_array().map(Vec::len), Some(2));
    assert_eq!(all[0]["chain"], "demo");

    let patched = api
        .call(
            Method::PATCH,
            "/v1/tickets/DEMO-2",
            Some(json!({ "title": "B2", "needs_secrets": true })),
        )
        .await
        .1;
    assert_eq!(
        (
            patched["ticket"]["title"].as_str(),
            patched["ticket"]["needs_secrets"].as_bool()
        ),
        (Some("B2"), Some(true))
    );
    assert_eq!(
        api.get("/v1/tickets/demo-2").await["ticket"]["title"],
        "B2",
        "keys are case-insensitive"
    );

    let (s, _) = api
        .call(
            Method::POST,
            "/v1/chains/demo/tickets",
            Some(json!({ "title": "C", "after": ["DEMO-9"] })),
        )
        .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_card_keeps_the_place_it_was_put_at_until_the_chain_is_arranged() {
    let api = Api::new();
    api.demo().await;
    api.post(
        "/v1/chains/demo/tickets",
        json!({ "title": "A", "paused": true }),
    )
    .await;
    api.post(
        "/v1/chains/demo/tickets",
        json!({ "title": "B", "paused": true }),
    )
    .await;
    assert!(
        api.get("/v1/tickets/DEMO-1").await["ticket"]
            .get("pos")
            .is_none()
    );

    let (s, moved) = api
        .call(
            Method::PATCH,
            "/v1/tickets/DEMO-1",
            Some(json!({ "pos": { "x": 248, "y": 40 } })),
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{moved}");
    assert_eq!(moved["ticket"]["pos"], json!({ "x": 248, "y": 40 }));
    api.call(
        Method::PATCH,
        "/v1/tickets/DEMO-2",
        Some(json!({ "pos": { "x": 8, "y": 8 } })),
    )
    .await;
    // A patch without `pos` keeps it.
    api.call(
        Method::PATCH,
        "/v1/tickets/DEMO-1",
        Some(json!({ "title": "A2" })),
    )
    .await;
    let chain = api.get("/v1/chains/demo").await;
    let at = |i: usize| chain["tickets"][i]["pos"].clone();
    assert_eq!(
        (at(0), at(1)),
        (json!({ "x": 248, "y": 40 }), json!({ "x": 8, "y": 8 }))
    );

    // `null` hands one card back to the layout.
    let (_, cleared) = api
        .call(
            Method::PATCH,
            "/v1/tickets/DEMO-2",
            Some(json!({ "pos": null })),
        )
        .await;
    assert!(cleared["ticket"].get("pos").is_none(), "{cleared}");
    assert_eq!(
        api.get("/v1/tickets/DEMO-1").await["ticket"]["pos"]["x"],
        248
    );

    // A size is kept like a place: a patch without it keeps it, `null` clears it.
    let (s, sized) = api
        .call(
            Method::PATCH,
            "/v1/tickets/DEMO-1",
            Some(json!({ "size": { "w": 300, "h": 120 } })),
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{sized}");
    assert_eq!(sized["ticket"]["size"], json!({ "w": 300, "h": 120 }));
    assert_eq!(sized["ticket"]["pos"], json!({ "x": 248, "y": 40 }));
    assert!(sized.get("size_clamped").is_none(), "{sized}");
    // A size out of range is stored clamped, and the answer says what was asked.
    let (s, big) = api
        .call(
            Method::PATCH,
            "/v1/tickets/DEMO-1",
            Some(json!({ "size": { "w": 9000, "h": 5 } })),
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{big}");
    assert_eq!(big["ticket"]["size"], json!({ "w": 480, "h": 5 }));
    assert_eq!(big["size_clamped"], json!({ "w": 9000, "h": 5 }));
    let (_, tall) = api
        .call(
            Method::PATCH,
            "/v1/tickets/DEMO-1",
            Some(json!({ "size": { "w": 300, "h": 120 } })),
        )
        .await;
    assert_eq!(tall["ticket"]["size"], json!({ "w": 300, "h": 120 }));
    api.call(
        Method::PATCH,
        "/v1/tickets/DEMO-1",
        Some(json!({ "title": "A3" })),
    )
    .await;
    assert_eq!(
        api.get("/v1/tickets/DEMO-1").await["ticket"]["size"]["w"],
        300
    );
    let (_, unsized_) = api
        .call(
            Method::PATCH,
            "/v1/tickets/DEMO-1",
            Some(json!({ "size": null })),
        )
        .await;
    assert!(unsized_["ticket"].get("size").is_none(), "{unsized_}");
    api.call(
        Method::PATCH,
        "/v1/tickets/DEMO-1",
        Some(json!({ "size": { "w": 300, "h": 120 } })),
    )
    .await;

    // The positions and sizes are not in the chain file.
    let toml = api.get("/v1/chains/demo/export").await["toml"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    assert!(!toml.contains("248"), "{toml}");

    api.call(
        Method::PATCH,
        "/v1/tickets/DEMO-2",
        Some(json!({ "pos": { "x": 8, "y": 8 } })),
    )
    .await;
    let (s, arranged) = api
        .call(Method::POST, "/v1/chains/demo/arrange", None)
        .await;
    assert_eq!(s, StatusCode::OK, "{arranged}");
    assert!(
        arranged["tickets"]
            .as_array()
            .expect("tickets")
            .iter()
            .all(|t| t.get("pos").is_none() && t.get("size").is_none())
    );
    let (s, _) = api
        .call(Method::POST, "/v1/chains/nope/arrange", None)
        .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn edges_add_remove_and_cycles() {
    let api = Api::new();
    api.demo().await;
    for t in ["A", "B"] {
        api.post("/v1/chains/demo/tickets", json!({ "title": t }))
            .await;
    }
    // Both started (no edges). Pause B, add the edge, resume: B waits for A.
    api.post("/v1/tickets/DEMO-2/actions/pause", json!({}))
        .await;
    let c = api
        .post(
            "/v1/chains/demo/edges",
            json!({ "from": "DEMO-1", "to": "DEMO-2" }),
        )
        .await;
    assert_eq!(c["edges"].as_array().map(Vec::len), Some(1));
    api.post("/v1/tickets/DEMO-2/actions/resume", json!({}))
        .await;
    assert_eq!(api.status("DEMO-2").await, "backlog");

    let (s, e) = api
        .call(
            Method::POST,
            "/v1/chains/demo/edges",
            Some(json!({ "from": "DEMO-2", "to": "DEMO-1" })),
        )
        .await;
    assert_eq!(s, StatusCode::CONFLICT);
    assert_eq!(e["error"], "dependency cycle: DEMO-1 -> DEMO-2 -> DEMO-1");

    let (s, c) = api
        .call(
            Method::DELETE,
            "/v1/chains/demo/edges?from=DEMO-1&to=DEMO-2",
            None,
        )
        .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(c["edges"].as_array().map(Vec::len), Some(0));
    assert_eq!(
        api.status("DEMO-2").await,
        "running",
        "no deps left: autostart"
    );
}

#[tokio::test]
async fn ticket_actions_walk_the_state_machine() {
    let api = Api::new();
    api.demo().await;
    api.post("/v1/chains/demo/tickets", json!({ "title": "A" }))
        .await;
    let (s, e) = api
        .call(
            Method::POST,
            "/v1/tickets/DEMO-1/actions/block",
            Some(json!({})),
        )
        .await;
    assert_eq!(
        (s, e["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("block needs a text"))
    );
    let t = api
        .post(
            "/v1/tickets/DEMO-1/actions/block",
            json!({ "text": "чекаємо ключ API" }),
        )
        .await;
    assert_eq!(t["ticket"]["status"], "blocked");
    assert_eq!(
        t["ticket"]["status_reason"]["reason"]["text"],
        "чекаємо ключ API"
    );
    assert_eq!(
        api.post("/v1/tickets/DEMO-1/actions/unblock", json!({}))
            .await["ticket"]["status"],
        "running"
    );
    let (s, e) = api
        .call(Method::POST, "/v1/tickets/DEMO-1/actions/accept", None)
        .await;
    assert_eq!(s, StatusCode::CONFLICT, "{e}");
    let (s, _) = api
        .call(Method::POST, "/v1/tickets/DEMO-1/actions/fly", None)
        .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let t = api
        .post(
            "/v1/tickets/DEMO-1/actions/cancel",
            json!({ "text": "won't do" }),
        )
        .await;
    assert_eq!(t["ticket"]["status"], "cancelled");
}

#[tokio::test]
async fn comments_are_delivered_to_sim_agents() {
    let api = Api::new();
    api.demo().await;
    api.post("/v1/chains/demo/tickets", json!({ "title": "A" }))
        .await;
    let t = api
        .post(
            "/v1/tickets/DEMO-1/comments",
            json!({ "text": "Solid\nand Vite" }),
        )
        .await;
    let c = &t["comments"][0];
    assert_eq!(
        (
            c["kind"].as_str(),
            c["body"]["gist"].as_str(),
            c["body"]["what"].as_str()
        ),
        (Some("instruction"), Some("Solid"), Some("and Vite"))
    );
    assert!(
        c["delivered_at"].is_number(),
        "running sim ticket reads it at once: {c}"
    );

    let decision =
        json!({ "kind": "decision", "body": { "gist": "rusqlite", "why": "one writer" } });
    let t = api.post("/v1/tickets/DEMO-1/comments", decision).await;
    assert_eq!(t["comments"][1]["body"]["why"], "one writer");
    let (s, _) = api
        .call(Method::POST, "/v1/tickets/DEMO-1/comments", Some(json!({})))
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn subtasks_add_and_tick() {
    let api = Api::new();
    api.demo().await;
    api.post("/v1/chains/demo/tickets", json!({ "title": "A" }))
        .await;
    api.post(
        "/v1/tickets/DEMO-1/subtasks",
        json!({ "text": "write test" }),
    )
    .await;
    let t = api
        .post(
            "/v1/tickets/DEMO-1/subtasks",
            json!({ "text": "write code" }),
        )
        .await;
    assert_eq!(t["subtasks"][1]["position"], 2);
    let (s, t) = api
        .call(
            Method::PATCH,
            "/v1/tickets/DEMO-1/subtasks/1",
            Some(json!({ "done": true })),
        )
        .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(
        (
            t["subtasks"][0]["done"].as_bool(),
            t["subtasks"][0]["done_by"].as_str()
        ),
        (Some(true), Some("human"))
    );
    let (s, _) = api
        .call(
            Method::PATCH,
            "/v1/tickets/DEMO-1/subtasks/9",
            Some(json!({ "done": true })),
        )
        .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn releases_create_start_complete() {
    let api = Api::new();
    api.demo().await;
    api.post("/v1/chains/demo/tickets", json!({ "title": "A" }))
        .await;
    let c = api
        .post(
            "/v1/chains/demo/releases",
            json!({ "version": "0.1.0", "after": ["DEMO-1"] }),
        )
        .await;
    assert_eq!(c["releases"][0]["state"], "planned");
    assert_eq!(c["releases"][0]["start_mode"], "manual");
    api.post(
        "/v1/chains/demo/tickets",
        json!({ "title": "C", "after": ["0.1.0"] }),
    )
    .await;
    let (s, e) = api
        .call(Method::POST, "/v1/releases/0.1.0/actions/start", None)
        .await;
    assert_eq!(s, StatusCode::CONFLICT);
    assert_eq!(e["error"], "release 0.1.0 still waits for: DEMO-1");

    api.post("/v1/tickets/DEMO-1/actions/pause", json!({}))
        .await;
    api.post("/v1/tickets/DEMO-1/actions/resume", json!({}))
        .await;
    // Drive A to Merged through the state machine: Running -> Review is an agent event,
    // so here cancel counts as done for the graph.
    api.post(
        "/v1/tickets/DEMO-1/actions/cancel",
        json!({ "text": "not needed" }),
    )
    .await;
    let c = api
        .post("/v1/releases/0.1.0/actions/start", json!({}))
        .await;
    assert_eq!(c["releases"][0]["state"], "cut");
    assert_eq!(
        api.status("DEMO-2").await,
        "running",
        "C starts after the cut"
    );
    let c = api
        .post("/v1/releases/0.1.0/actions/complete", json!({}))
        .await;
    assert_eq!(c["releases"][0]["state"], "released");
    let (s, _) = api
        .call(Method::POST, "/v1/releases/0.1.0/actions/complete", None)
        .await;
    assert_eq!(s, StatusCode::CONFLICT);
}

#[tokio::test]
async fn events_journal_every_transition_in_order() {
    let api = Api::new();
    api.demo().await;
    api.post("/v1/chains/demo/tickets", json!({ "title": "A" }))
        .await;
    let events = api.get("/v1/events?since=0").await;
    let kinds: Vec<String> = events
        .as_array()
        .expect("array")
        .iter()
        .map(|e| {
            let to = e["payload"]["to"]
                .as_str()
                .map(|t| format!(":{t}"))
                .unwrap_or_default();
            format!(
                "{}.{}{to}",
                e["entity"].as_str().unwrap_or(""),
                e["kind"].as_str().unwrap_or("")
            )
        })
        .collect();
    assert_eq!(
        kinds,
        [
            "project.created",
            "chain.created",
            "ticket.created",
            "ticket.status_changed:ready",
            "ticket.status_changed:running"
        ]
    );
    let seqs: Vec<i64> = events
        .as_array()
        .expect("array")
        .iter()
        .filter_map(|e| e["seq"].as_i64())
        .collect();
    assert_eq!(seqs, vec![1, 2, 3, 4, 5]);
    assert_eq!(
        api.get("/v1/events?since=3").await.as_array().map(Vec::len),
        Some(2)
    );
}

#[tokio::test]
async fn wildcard_cards_are_proposed_then_approved_or_rejected_by_a_person() {
    let api = Api::new();
    api.demo().await;
    api.post("/v1/chains/demo/tickets", json!({ "title": "A" }))
        .await;
    api.post(
        "/v1/chains/demo/tickets",
        json!({ "title": "B", "after": ["DEMO-1"] }),
    )
    .await;

    // A person proposes; the answer is the card, 201.
    let body =
        json!({ "title": "refresh", "after": ["DEMO-1"], "before": ["DEMO-2"], "from": "DEMO-1" });
    let (s, card) = api
        .call(Method::POST, "/v1/chains/demo/wildcards", Some(body))
        .await;
    assert_eq!(s, StatusCode::CREATED, "{card}");
    assert_eq!(card["ticket"]["key"], "DEMO-3");
    assert_eq!(card["ticket"]["status"], "proposed");
    assert_eq!(card["ticket"]["wildcard"]["proposed_by"], "human");
    assert_eq!(card["ticket"]["wildcard"]["from"], "DEMO-1");

    // An agent proposes through its event; the answer is the new card, found during its ticket.
    let agent = api
        .post(
            "/v1/tickets/DEMO-1/events",
            json!({ "event": "propose", "title": "docs" }),
        )
        .await;
    assert_eq!(agent["ticket"]["key"], "DEMO-4");
    assert_eq!(agent["ticket"]["wildcard"]["proposed_by"], "agent");
    assert_eq!(agent["ticket"]["wildcard"]["from"], "DEMO-1");

    let human = api.get("/v1/tickets?needs_human=true").await;
    let keys: Vec<&str> = human
        .as_array()
        .expect("array")
        .iter()
        .filter_map(|t| t["ticket"]["key"].as_str())
        .collect();
    assert_eq!(keys, ["DEMO-3", "DEMO-4"]);
    let chain = api.get("/v1/chains/demo").await;
    assert_eq!(
        chain["summary"]["total"], 2,
        "proposals are not in the plan"
    );

    // A person approves and rejects through the ticket actions.
    let t = api
        .post("/v1/tickets/DEMO-3/actions/approve", json!({}))
        .await;
    assert_eq!(t["ticket"]["status"], "backlog");
    assert_eq!(t["ticket"]["wildcard"]["approved"], true);
    let (s, e) = api
        .call(
            Method::POST,
            "/v1/tickets/DEMO-4/actions/reject",
            Some(json!({})),
        )
        .await;
    assert_eq!(
        (s, e["code"].as_str()),
        (StatusCode::BAD_REQUEST, Some("bad_request"))
    );
    let t = api
        .post(
            "/v1/tickets/DEMO-4/actions/reject",
            json!({ "text": "later" }),
        )
        .await;
    assert_eq!(t["ticket"]["status"], "cancelled");
    let (s, e) = api
        .call(
            Method::POST,
            "/v1/tickets/DEMO-4/actions/approve",
            Some(json!({})),
        )
        .await;
    assert_eq!(s, StatusCode::CONFLICT);
    assert_eq!(e["error"], "cannot approve a ticket in Cancelled");

    // Refused proposals change nothing.
    let (s, _) = api
        .call(
            Method::POST,
            "/v1/chains/demo/wildcards",
            Some(json!({ "title": " " })),
        )
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let (s, _) = api
        .call(
            Method::POST,
            "/v1/chains/nope/wildcards",
            Some(json!({ "title": "x" })),
        )
        .await;
    assert_eq!(s, StatusCode::NOT_FOUND);

    // The journal says who did what.
    let events = api.get("/v1/events?since=0").await;
    let decisions: Vec<String> = events
        .as_array()
        .expect("array")
        .iter()
        .filter(|e| {
            matches!(
                e["kind"].as_str(),
                Some("proposed" | "approved" | "rejected")
            )
        })
        .map(|e| {
            format!(
                "{} {} {}",
                e["actor"].as_str().unwrap_or(""),
                e["payload"]["ticket"]["key"].as_str().unwrap_or(""),
                e["kind"].as_str().unwrap_or("")
            )
        })
        .collect();
    assert_eq!(
        decisions,
        [
            "human DEMO-3 proposed",
            "agent DEMO-4 proposed",
            "human DEMO-3 approved",
            "human DEMO-4 rejected"
        ]
    );
}

#[tokio::test]
async fn openapi_describes_every_route() {
    let api = Api::new();
    let req = Request::builder()
        .uri("/openapi.json")
        .body(Body::empty())
        .expect("request");
    let resp = api.app.clone().oneshot(req).await.expect("response");
    assert_eq!(resp.status(), StatusCode::OK, "the description is public");
    let bytes = resp.into_body().collect().await.expect("body").to_bytes();
    let doc: Value = serde_json::from_slice(&bytes).expect("json");
    let mut paths: Vec<String> = doc["paths"]
        .as_object()
        .expect("paths")
        .keys()
        .cloned()
        .collect();
    paths.sort();
    assert_eq!(
        paths,
        [
            "/agents",
            "/chains",
            "/chains/{chain}",
            "/chains/{chain}/arrange",
            "/chains/{chain}/edges",
            "/chains/{chain}/edges/move",
            "/chains/{chain}/export",
            "/chains/{chain}/move",
            "/chains/{chain}/release",
            "/chains/{chain}/releases",
            "/chains/{chain}/tickets",
            "/chains/{chain}/tickets/{ticket}/move",
            "/chains/{chain}/wildcards",
            "/events",
            "/planner/cards/{card}",
            "/planner/cards/{card}/show",
            "/planner/cards/{card}/threads",
            "/planner/cards/{card}/threads/{thread}/messages",
            "/planner/cards/{card}/threads/{thread}/versions",
            "/planner/chains",
            "/planner/chains/{chain}",
            "/planner/chains/{chain}/cards",
            "/planner/chains/{chain}/cards/{card}/move",
            "/planner/chains/{chain}/edges",
            "/planner/designs/{design}",
            "/planner/overview",
            "/planner/pins",
            "/planner/plan",
            "/planner/say",
            "/planner/suggestion",
            "/profiles",
            "/projects",
            "/projects/{project}",
            "/projects/{project}/archive",
            "/projects/{project}/chains",
            "/projects/{project}/conversation",
            "/projects/{project}/delete",
            "/projects/{project}/design-pins",
            "/projects/{project}/design-pins/{pin}/answer",
            "/projects/{project}/designs",
            "/projects/{project}/designs/figma",
            "/projects/{project}/designs/{design}",
            "/projects/{project}/designs/{design}/fetch",
            "/projects/{project}/designs/{design}/file",
            "/projects/{project}/designs/{design}/thumb",
            "/projects/{project}/figma-token",
            "/projects/{project}/import",
            "/projects/{project}/import-path",
            "/projects/{project}/inbox",
            "/projects/{project}/next-version",
            "/projects/{project}/planner",
            "/projects/{project}/planner/input",
            "/projects/{project}/planner/scrollback",
            "/projects/{project}/planner/size",
            "/projects/{project}/planner/start",
            "/projects/{project}/planner/stop",
            "/projects/{project}/plans",
            "/projects/{project}/strays",
            "/projects/{project}/strays/clean",
            "/projects/{project}/suggestion",
            "/projects/{project}/unarchive",
            "/releases/{release}/actions/{action}",
            "/releases/{release}/events",
            "/releases/{release}/run",
            "/repos/check",
            "/repos/prepare",
            "/system/checks",
            "/system/cli",
            "/system/cli/install",
            "/system/places",
            "/tickets",
            "/tickets/{ticket}",
            "/tickets/{ticket}/actions/{action}",
            "/tickets/{ticket}/activity",
            "/tickets/{ticket}/comments",
            "/tickets/{ticket}/designs/{index}/move",
            "/tickets/{ticket}/discussion",
            "/tickets/{ticket}/events",
            "/tickets/{ticket}/instructions/claim",
            "/tickets/{ticket}/leftovers/{action}",
            "/tickets/{ticket}/permission",
            "/tickets/{ticket}/pre-tool",
            "/tickets/{ticket}/runner/answer",
            "/tickets/{ticket}/runner/input",
            "/tickets/{ticket}/runner/scrollback",
            "/tickets/{ticket}/runner/size",
            "/tickets/{ticket}/subtasks",
            "/tickets/{ticket}/subtasks/{position}",
            "/tickets/{ticket}/threads",
            "/tickets/{ticket}/threads/{thread}/approve",
            "/tickets/{ticket}/threads/{thread}/changes",
            "/tickets/{ticket}/threads/{thread}/edits/{message}/accept",
            "/tickets/{ticket}/threads/{thread}/edits/{message}/reject",
            "/tickets/{ticket}/threads/{thread}/messages",
            "/tickets/{ticket}/threads/{thread}/versions",
            "/usage",
            "/usage/quota",
            "/usage/settings",
        ]
    );
    for schema in [
        "Ticket",
        "TicketStatus",
        "ChainSummary",
        "StatusReason",
        "ErrorBody",
        "EventRecord",
        "Wildcard",
        "NewWildcard",
        "Proposal",
        "Thread",
        "ThreadDetail",
        "TicketThreads",
        "Phase",
    ] {
        assert!(
            doc["components"]["schemas"][schema].is_object(),
            "schema {schema} missing"
        );
    }
    assert!(doc["components"]["securitySchemes"]["bearer"].is_object());
}

#[tokio::test]
async fn sim_agent_events_drive_a_ticket() {
    let api = Api::new();
    api.demo().await;
    let t = api
        .post(
            "/v1/chains/demo/tickets",
            json!({ "title": "A", "profile": "sim", "subtasks": ["test"] }),
        )
        .await;
    assert_eq!(
        t["ticket"]["status"], "running",
        "autostart with the sim profile"
    );
    let t = api
        .post("/v1/tickets/DEMO-1/events", json!({ "event": "started" }))
        .await;
    assert_eq!(
        t["ticket"]["status"], "running",
        "started on a running ticket is a no-op"
    );

    let t = api
        .post(
            "/v1/tickets/DEMO-1/events",
            json!({ "event": "ask_human", "text": "який фреймворк?" }),
        )
        .await;
    assert_eq!(t["ticket"]["status"], "waiting");
    assert_eq!(t["ticket"]["status_reason"]["text"], "який фреймворк?");
    assert_eq!(
        (
            t["comments"][0]["author"].as_str(),
            t["comments"][0]["kind"].as_str()
        ),
        (Some("agent"), Some("report"))
    );

    let t = api
        .post("/v1/tickets/DEMO-1/comments", json!({ "text": "Solid" }))
        .await;
    assert_eq!(t["ticket"]["status"], "running", "instruction delivered");
    assert!(t["comments"][1]["delivered_at"].is_number());

    let t = api
        .post(
            "/v1/tickets/DEMO-1/events",
            json!({ "event": "subtask_done", "position": 1 }),
        )
        .await;
    assert_eq!(t["subtasks"][0]["done_by"], "agent");
    let t = api
        .post(
            "/v1/tickets/DEMO-1/events",
            json!({ "event": "note", "body": { "gist": "half way" } }),
        )
        .await;
    assert_eq!(t["comments"][2]["body"]["gist"], "half way");
    let t = api
        .post(
            "/v1/tickets/DEMO-1/events",
            json!({ "event": "finished", "summary": toknila_core::handin::from_plain("зроблено X") }),
        )
        .await;
    assert_eq!(t["ticket"]["status"], "review");
    assert_eq!(t["comments"][3]["body"]["gist"], "зроблено X");

    let t = api
        .post(
            "/v1/tickets/DEMO-1/actions/return",
            json!({ "text": "додай тест" }),
        )
        .await;
    assert_eq!(t["ticket"]["status"], "running");
    let t = api
        .post(
            "/v1/tickets/DEMO-1/events",
            json!({ "event": "crashed", "detail": "exit 101" }),
        )
        .await;
    assert_eq!(t["ticket"]["status"], "blocked");
    assert_eq!(t["ticket"]["status_reason"]["reason"]["kind"], "crashed");

    let (s, _) = api
        .call(
            Method::POST,
            "/v1/tickets/DEMO-1/events",
            Some(json!({ "event": "teleport" })),
        )
        .await;
    assert!(s.is_client_error());
}

#[tokio::test]
async fn release_agent_finishes_a_cut_release() {
    let api = Api::new();
    api.demo().await;
    api.post("/v1/chains/demo/tickets", json!({ "title": "A" }))
        .await;
    api.post(
        "/v1/chains/demo/releases",
        json!({ "version": "0.1.0", "after": ["DEMO-1"] }),
    )
    .await;
    api.post(
        "/v1/tickets/DEMO-1/events",
        json!({ "event": "finished", "summary": toknila_core::handin::from_plain("done") }),
    )
    .await;
    api.post("/v1/tickets/DEMO-1/actions/accept", json!({}))
        .await;
    let (s, _) = api
        .call(
            Method::POST,
            "/v1/releases/0.1.0/events",
            Some(json!({ "event": "finished" })),
        )
        .await;
    assert_eq!(s, StatusCode::CONFLICT, "not cut yet");
    api.post("/v1/releases/0.1.0/actions/start", json!({}))
        .await;
    let c = api
        .post("/v1/releases/0.1.0/events", json!({ "event": "finished" }))
        .await;
    assert_eq!(c["releases"][0]["state"], "released");
    assert_eq!(c["tickets"][0]["status"], "released");
}

const DEMO_TOML: &str = include_str!("../../../../examples/demo.toml");

/// Edges as "from>to" by ticket title and release version, sorted.
fn edge_titles(detail: &Value) -> Vec<String> {
    let name = |n: &Value| -> String {
        let id = &n["id"];
        let kind = n["kind"].as_str().unwrap_or_default();
        let list = if kind == "ticket" {
            "tickets"
        } else {
            "releases"
        };
        let field = if kind == "ticket" { "title" } else { "version" };
        detail[list]
            .as_array()
            .and_then(|a| a.iter().find(|x| &x["id"] == id))
            .map(|x| x[field].as_str().unwrap_or_default().to_owned())
            .unwrap_or_default()
    };
    let mut e: Vec<String> = detail["edges"]
        .as_array()
        .expect("edges")
        .iter()
        .map(|e| format!("{}>{}", name(&e["from"]), name(&e["to"])))
        .collect();
    e.sort();
    e
}

#[tokio::test]
async fn import_export_roundtrip() {
    let api = Api::new();
    api.post("/v1/projects", json!({ "key": "DEMO" })).await;
    api.post("/v1/projects", json!({ "key": "COPY" })).await;
    let (s, first) = api
        .call(
            Method::POST,
            "/v1/projects/DEMO/import",
            Some(json!({ "toml": DEMO_TOML })),
        )
        .await;
    assert_eq!(s, StatusCode::CREATED, "{first}");
    assert_eq!(edge_titles(&first), ["A>B", "A>D", "B>R1", "D>R1", "R1>C"]);
    assert_eq!(
        api.status("DEMO-1").await,
        "running",
        "autostart after import"
    );
    assert_eq!(api.status("DEMO-2").await, "backlog");
    let a = api.get("/v1/tickets/DEMO-1").await;
    assert_eq!(a["subtasks"][1]["text"], "make it pass");

    let exported = api.get("/v1/chains/demo/export").await;
    let text = exported["toml"].as_str().expect("toml").to_owned();
    println!("{text}");
    assert!(text.contains("id = \"DEMO-1\""), "{text}");

    let second = api
        .post("/v1/projects/COPY/import", json!({ "toml": text }))
        .await;
    assert_eq!(edge_titles(&first), edge_titles(&second));
    let titles = |d: &Value| -> Vec<String> {
        let mut t: Vec<String> = d["tickets"]
            .as_array()
            .expect("tickets")
            .iter()
            .map(|t| format!("{} {}", t["title"], t["profile"]))
            .collect();
        t.sort();
        t
    };
    assert_eq!(titles(&first), titles(&second));
    let c = api.get("/v1/tickets/COPY-1").await;
    assert_eq!(c["ticket"]["title"], "A");
    assert_eq!(c["subtasks"].as_array().map(Vec::len), Some(2));
}

#[tokio::test]
async fn import_rejects_bad_files_and_leaves_nothing_behind() {
    let api = Api::new();
    api.post("/v1/projects", json!({ "key": "DEMO" })).await;
    let bad = "[chain]\nname = \"x\"\n[[ticket]]\nid = \"A\"\nafter = [\"nope\"]\n";
    let (s, v) = api
        .call(
            Method::POST,
            "/v1/projects/DEMO/import",
            Some(json!({ "toml": bad })),
        )
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert_eq!(v["error"], "line 5: unknown id \"nope\"", "{v}");

    // Fails inside the transaction, after the chain row was written: all of it rolls back.
    let late = "[chain]\nname = \"late\"\n[[ticket]]\nid = \"A\"\nsubtasks = [\"ok\", \" \"]\n";
    let (s, v) = api
        .call(
            Method::POST,
            "/v1/projects/DEMO/import",
            Some(json!({ "toml": late })),
        )
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{v}");
    assert_eq!(api.get("/v1/chains").await, json!([]));
    assert_eq!(api.get("/v1/tickets").await, json!([]));

    api.post("/v1/projects/DEMO/import", json!({ "toml": DEMO_TOML }))
        .await;
    let (s, _) = api
        .call(
            Method::POST,
            "/v1/projects/DEMO/import",
            Some(json!({ "toml": DEMO_TOML })),
        )
        .await;
    assert_eq!(s, StatusCode::CONFLICT, "same chain name twice");
}

#[tokio::test]
async fn page_routes_serve_the_page_and_nothing_else_does() {
    let api = Api::new();
    let status = |uri: &'static str| {
        let app = api.app.clone();
        async move {
            let req = Request::builder()
                .uri(uri)
                .body(Body::empty())
                .expect("request");
            let resp = app.oneshot(req).await.expect("response");
            let ct = resp
                .headers()
                .get("content-type")
                .map(|v| v.to_str().unwrap_or("").to_owned())
                .unwrap_or_default();
            (resp.status(), ct)
        }
    };
    let html = (StatusCode::OK, "text/html; charset=utf-8".to_owned());
    assert_eq!(status("/chains/demo").await, html);
    assert_eq!(status("/inbox").await, html);
    assert_eq!(status("/projects/DEMO").await, html);
    // Every settings section has an address of its own.
    for page in [
        "/settings",
        "/settings/profiles",
        "/settings/updates",
        "/settings/about",
        "/settings/checks",
        "/projects/DEMO/settings",
        "/projects/DEMO/settings/agents",
        "/projects/DEMO/settings/advanced",
    ] {
        assert_eq!(status(page).await, html, "{page}");
    }
    assert_eq!(
        status("/projects/DEMO/settings/agents/extra").await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        status("/settings/checks/extra").await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(status("/projects/DEMO/chains/demo").await, html);
    assert_eq!(
        status("/projects/DEMO/chains/demo/cards/DEMO-1").await,
        html
    );
    assert_eq!(status("/chains/demo/extra").await.0, StatusCode::NOT_FOUND);
    assert_eq!(
        status("/projects/DEMO/chains/demo/extra").await.0,
        StatusCode::NOT_FOUND
    );
    let (code, ct) = status("/v1/nowhere").await;
    assert_eq!(code, StatusCode::NOT_FOUND);
    assert!(!ct.starts_with("text/html"), "{ct}");
}

#[tokio::test]
async fn web_page_is_served_without_a_token() {
    let api = Api::new();
    let get = |uri: &str| {
        Request::builder()
            .uri(uri)
            .body(Body::empty())
            .expect("request")
    };
    let resp = api.app.clone().oneshot(get("/")).await.expect("response");
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(resp.headers()["content-type"], "text/html; charset=utf-8");
    let html = String::from_utf8(
        resp.into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes()
            .to_vec(),
    )
    .expect("utf-8");

    if std::env::var_os("TOKNILA_REQUIRE_WEB").is_some() {
        assert!(
            crate::web::embedded(),
            "CI builds apps/web before the daemon"
        );
    }
    if !crate::web::embedded() {
        assert!(html.contains("cargo xtask web"), "{html}");
        return;
    }
    assert!(html.contains(r#"<div id="root">"#), "{html}");
    let script = html
        .split(r#"src=""#)
        .nth(1)
        .and_then(|s| s.split('"').next())
        .expect("index.html loads a script");
    assert!(script.starts_with("/assets/"), "{script}");
    let resp = api
        .app
        .clone()
        .oneshot(get(script))
        .await
        .expect("response");
    assert_eq!(resp.status(), StatusCode::OK, "{script}");
    assert_eq!(
        resp.headers()["content-type"],
        "text/javascript; charset=utf-8"
    );
    let resp = api
        .app
        .clone()
        .oneshot(get("/assets/missing.js"))
        .await
        .expect("response");
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

/// The web page's TypeScript types are generated from this document (`npm run gen` in
/// `apps/web`), so it is pinned: a change to the API shows up here first.
#[test]
fn openapi_document_matches_the_committed_copy() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps/web/openapi.json");
    let now = format!("{}\n", super::openapi_json());
    if std::env::var_os("TOKNILA_UPDATE_SNAPSHOTS").is_some() {
        std::fs::write(&path, &now).expect("write openapi.json");
        return;
    }
    let committed = std::fs::read_to_string(&path)
        .expect("apps/web/openapi.json is missing; run with TOKNILA_UPDATE_SNAPSHOTS=1")
        .replace("\r\n", "\n");
    assert!(
        committed == now,
        "the API document changed; if intended, rerun with TOKNILA_UPDATE_SNAPSHOTS=1 and run \
         `npm run gen` in apps/web"
    );
}
