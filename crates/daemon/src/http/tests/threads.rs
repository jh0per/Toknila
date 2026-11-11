//! Threads on a card through the full router: a person's routes, the agent's events, the
//! hand-in gate, and the journal.

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};

use super::Api;

async fn kinds(api: &Api, entity: &str) -> Vec<String> {
    api.get("/v1/events")
        .await
        .as_array()
        .expect("events")
        .iter()
        .filter(|e| e["entity"] == entity)
        .map(|e| e["kind"].as_str().expect("kind").to_owned())
        .collect()
}

#[tokio::test]
async fn a_person_and_the_agent_settle_a_thread_and_the_hand_in_follows() {
    let api = Api::new();
    api.demo().await;
    api.post("/v1/chains/demo/tickets", json!({ "title": "A" }))
        .await;
    assert_eq!(api.status("DEMO-1").await, "running");

    // The agent opens a thread and posts version 1 through its events.
    let ev = |v: Value| api.post("/v1/tickets/DEMO-1/events", v);
    let d = ev(json!({ "event": "open_thread", "kind": "design", "title": "List screen" })).await;
    assert_eq!(d["phase"], "design");
    ev(json!({ "event": "thread_version", "thread": "1", "format": "mermaid", "body": "graph TD; A-->B" }))
        .await;
    ev(json!({ "event": "thread_message", "thread": "1", "text": "states follow" })).await;

    let (s, err) = api
        .call(
            Method::POST,
            "/v1/tickets/DEMO-1/events",
            Some(
                json!({ "event": "finished", "summary": toknila_core::handin::from_plain("done") }),
            ),
        )
        .await;
    assert_eq!(s, StatusCode::CONFLICT, "{err}");
    assert_eq!(err["code"], "conflict");
    assert_eq!(
        err["error"],
        "cannot hand in DEMO-1: 1 open: #1 \"List screen\" (Design)"
    );
    assert_eq!(
        api.status("DEMO-1").await,
        "running",
        "the ticket did not move"
    );

    // The list: the thread with its message and version, and the computed phase.
    let list = api.get("/v1/tickets/DEMO-1/threads").await;
    assert_eq!(
        (&list["ticket"], &list["phase"]),
        (&json!("DEMO-1"), &json!("design"))
    );
    let th = &list["threads"][0];
    assert_eq!(th["messages"].as_array().map(Vec::len), Some(1));
    assert_eq!(th["versions"][0]["body"], "graph TD; A-->B");
    assert_eq!(th["changes_requested"], false);
    let id = th["thread"]["id"].as_str().expect("id").to_owned();

    // A person asks for changes and approves; the thread can be addressed by number or id.
    let t = api
        .post(
            "/v1/tickets/DEMO-1/threads/1/changes",
            json!({ "text": "add an error state" }),
        )
        .await;
    assert_eq!(t["thread"]["state"], "open");
    assert_eq!(t["changes_requested"], true);
    // Timestamps are milliseconds; a version in the same one would not answer the request.
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    ev(json!({ "event": "thread_version", "thread": id, "format": "mermaid", "body": "graph TD; A-->E" }))
        .await;
    let (s, v) = api
        .call(
            Method::POST,
            "/v1/tickets/DEMO-1/threads/1/versions",
            Some(json!({ "format": "markdown", "body": "# Notes" })),
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["versions"].as_array().map(Vec::len), Some(3));
    // New versions answered the request: still open, no longer waiting on changes.
    assert_eq!(
        (&v["thread"]["state"], &v["changes_requested"]),
        (&json!("open"), &json!(false))
    );
    let approved = api
        .post("/v1/tickets/DEMO-1/threads/1/approve", json!({}))
        .await;
    assert_eq!(
        (
            &approved["thread"]["state"],
            &approved["thread"]["approved_version"]
        ),
        (&json!("approved"), &json!(3))
    );
    assert_eq!(approved["changes_requested"], false);
    let (s, again) = api
        .call(Method::POST, "/v1/tickets/DEMO-1/threads/1/approve", None)
        .await;
    assert_eq!(
        (s, &again["error"]),
        (StatusCode::CONFLICT, &json!("thread is already approved"))
    );

    let done =
        ev(json!({ "event": "finished", "summary": toknila_core::handin::from_plain("done") }))
            .await;
    assert_eq!(
        (&done["ticket"]["status"], &done["phase"]),
        (&json!("review"), &json!("implement"))
    );

    // Every step is in the journal, typed.
    assert_eq!(
        kinds(&api, "thread").await,
        [
            "opened",
            "version_posted",
            "message_posted",
            "message_posted",
            "version_posted",
            "version_posted",
            "approved"
        ]
    );
}

#[tokio::test]
async fn bad_input_is_refused_with_a_typed_error_and_stores_nothing() {
    let api = Api::new();
    api.demo().await;
    api.post("/v1/chains/demo/tickets", json!({ "title": "A" }))
        .await;
    let (s, e) = api
        .call(
            Method::POST,
            "/v1/tickets/DEMO-1/threads",
            Some(json!({ "kind": "design", "title": " " })),
        )
        .await;
    assert_eq!(
        (s, &e["code"]),
        (StatusCode::BAD_REQUEST, &json!("bad_request"))
    );
    let (s, e) = api
        .call(
            Method::POST,
            "/v1/tickets/DEMO-1/threads",
            Some(json!({ "kind": "poster", "title": "x" })),
        )
        .await;
    assert!(s.is_client_error(), "{e}");
    let (s, e) = api
        .call(Method::POST, "/v1/tickets/DEMO-1/threads/3/approve", None)
        .await;
    assert_eq!(
        (s, &e["error"]),
        (
            StatusCode::NOT_FOUND,
            &json!("thread 3 of DEMO-1 not found")
        )
    );

    let t = api
        .post(
            "/v1/tickets/DEMO-1/threads",
            json!({ "kind": "architecture", "title": "Modules" }),
        )
        .await;
    assert_eq!(t["thread"]["opened_by"], "human");
    let big = "x".repeat(512 * 1024 + 1);
    let (s, e) = api
        .call(
            Method::POST,
            "/v1/tickets/DEMO-1/threads/1/versions",
            Some(json!({ "format": "html", "body": big })),
        )
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{e}");
    assert_eq!(e["error"], "artifact is 524289 bytes, the limit is 524288");
    let list = api.get("/v1/tickets/DEMO-1/threads").await;
    assert_eq!(list["threads"][0]["versions"], json!([]));
    assert_eq!(kinds(&api, "thread").await, ["opened"]);
}

#[tokio::test]
async fn approving_a_thread_answers_the_question_the_worker_waits_on() {
    let api = Api::new();
    api.demo().await;
    api.post(
        "/v1/chains/demo/tickets",
        json!({ "title": "A", "profile": "sim" }),
    )
    .await;
    let ev = |v: Value| api.post("/v1/tickets/DEMO-1/events", v);
    ev(json!({ "event": "open_thread", "kind": "design", "title": "List screen" })).await;
    ev(json!({ "event": "thread_version", "thread": "1", "format": "mermaid", "body": "graph TD; A-->B" }))
        .await;
    ev(json!({ "event": "ask_human", "text": "Design v1 is in thread 1. Do you approve it?" }))
        .await;
    assert_eq!(api.status("DEMO-1").await, "waiting");
    let inbox = api.get("/v1/projects/DEMO/inbox").await;
    let kinds: Vec<&Value> = inbox
        .as_array()
        .expect("list")
        .iter()
        .map(|i| &i["kind"])
        .collect();
    assert_eq!(kinds, ["question", "thread"], "{inbox}");

    api.post(
        "/v1/tickets/DEMO-1/threads/1/messages",
        json!({ "text": "Looks right" }),
    )
    .await;
    api.post("/v1/tickets/DEMO-1/threads/1/approve", json!({}))
        .await;

    assert_eq!(api.status("DEMO-1").await, "running");
    assert_eq!(api.get("/v1/projects/DEMO/inbox").await, json!([]));
    let card = api.get("/v1/tickets/DEMO-1").await;
    let text = card.to_string();
    assert!(
        text.contains(r#"Thread 1 \"List screen\" version 1 is approved. Looks right. Continue."#),
        "{text}"
    );
}

#[tokio::test]
async fn a_second_thread_of_a_kind_is_refused_with_a_hint_naming_the_first() {
    let api = Api::new();
    api.demo().await;
    api.post("/v1/chains/demo/tickets", json!({ "title": "A" }))
        .await;
    let open = |kind: &str| {
        api.call(
            Method::POST,
            "/v1/tickets/DEMO-1/events",
            Some(json!({ "event": "open_thread", "kind": kind, "title": "Screen" })),
        )
    };
    assert_eq!(open("design").await.0, StatusCode::OK);
    let (s, err) = open("design").await;
    assert_eq!(s, StatusCode::CONFLICT, "{err}");
    let hint = err["error"].as_str().expect("error");
    assert!(
        hint.contains("thread 1") && hint.contains("post a version"),
        "{hint}"
    );
    // Control: another kind is fine, and the refused call left no thread behind.
    assert_eq!(open("flow").await.0, StatusCode::OK);
    let list = api.get("/v1/tickets/DEMO-1/threads").await;
    assert_eq!(list["threads"].as_array().expect("threads").len(), 2);
}

#[tokio::test]
async fn an_agent_withdraws_its_empty_thread_and_it_never_blocks_finished() {
    let api = Api::new();
    api.demo().await;
    api.post("/v1/chains/demo/tickets", json!({ "title": "A" }))
        .await;
    let ev = |v: Value| api.call(Method::POST, "/v1/tickets/DEMO-1/events", Some(v));
    ev(json!({ "event": "open_thread", "kind": "design", "title": "Screen" })).await;
    ev(json!({ "event": "open_thread", "kind": "flow", "title": "Steps" })).await;
    // An empty thread does not block the hand-in.
    let done = json!({ "event": "finished", "summary": toknila_core::handin::from_plain("done") });

    // A thread with a person's message cannot be withdrawn.
    api.post(
        "/v1/tickets/DEMO-1/threads/2/messages",
        json!({ "text": "wait" }),
    )
    .await;
    let (s, err) = ev(json!({ "event": "withdraw_thread", "thread": "2" })).await;
    assert_eq!(s, StatusCode::CONFLICT, "{err}");

    let (s, _) = ev(json!({ "event": "withdraw_thread", "thread": "1" })).await;
    assert_eq!(s, StatusCode::OK);
    let list = api.get("/v1/tickets/DEMO-1/threads").await;
    assert_eq!(list["threads"][0]["thread"]["state"], "withdrawn");
    assert_eq!(
        kinds(&api, "thread").await.last().map(String::as_str),
        Some("withdrawn")
    );

    // A thread with a version cannot be withdrawn and does block.
    ev(json!({ "event": "thread_version", "thread": "2", "format": "mermaid", "body": "graph TD" }))
        .await;
    let (s, _) = ev(json!({ "event": "withdraw_thread", "thread": "2" })).await;
    assert_eq!(s, StatusCode::CONFLICT);
    assert_eq!(ev(done.clone()).await.0, StatusCode::CONFLICT);
    api.post("/v1/tickets/DEMO-1/threads/2/approve", json!({}))
        .await;
    assert_eq!(ev(done).await.0, StatusCode::OK);
    assert_eq!(api.status("DEMO-1").await, "review");
}

#[tokio::test]
async fn an_empty_thread_alone_does_not_stop_finished() {
    let api = Api::new();
    api.demo().await;
    api.post("/v1/chains/demo/tickets", json!({ "title": "A" }))
        .await;
    let ev = |v: Value| api.call(Method::POST, "/v1/tickets/DEMO-1/events", Some(v));
    ev(json!({ "event": "open_thread", "kind": "design", "title": "Screen" })).await;
    let done = json!({ "event": "finished", "summary": toknila_core::handin::from_plain("done") });
    assert_eq!(ev(done).await.0, StatusCode::OK);
    assert_eq!(api.status("DEMO-1").await, "review");
}
