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
