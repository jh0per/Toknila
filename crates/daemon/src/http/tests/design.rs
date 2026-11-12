//! "Design first" through the full router: the flag on create, patch, import and export, the
//! `ask_design` action in each state, and the hand-in gate.

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};

use super::Api;

fn offers(card: &Value, action: &str) -> bool {
    card["allowed_actions"]
        .as_array()
        .expect("allowed_actions")
        .iter()
        .any(|a| a == action)
}

#[tokio::test]
async fn the_flag_is_set_on_create_and_patch_and_travels_through_a_chain_file() {
    let api = Api::new();
    api.post("/v1/projects", json!({ "key": "DEMO" })).await;
    api.post("/v1/projects", json!({ "key": "COPY" })).await;
    api.post("/v1/projects/DEMO/chains", json!({ "name": "demo" }))
        .await;
    let plain = api
        .post(
            "/v1/chains/demo/tickets",
            json!({ "title": "A", "paused": true }),
        )
        .await;
    assert!(
        plain["ticket"].get("design_first").is_none(),
        "off by default and left out: {plain}"
    );
    let asked = api
        .post(
            "/v1/chains/demo/tickets",
            json!({ "title": "B", "paused": true, "design_first": true }),
        )
        .await;
    assert_eq!(asked["ticket"]["design_first"], true);
    assert!(!offers(&asked, "ask_design"), "already asked for");
    assert!(offers(&plain, "ask_design"));

    let patched = api
        .call(
            Method::PATCH,
            "/v1/tickets/DEMO-1",
            Some(json!({ "design_first": true })),
        )
        .await
        .1;
    assert_eq!(patched["ticket"]["design_first"], true);
    let off = api
        .call(
            Method::PATCH,
            "/v1/tickets/DEMO-1",
            Some(json!({ "design_first": false })),
        )
        .await
        .1;
    assert!(off["ticket"].get("design_first").is_none());

    let text = api.get("/v1/chains/demo/export").await["toml"]
        .as_str()
        .expect("toml")
        .to_owned();
    assert_eq!(text.matches("design_first = true").count(), 1, "{text}");
    api.post("/v1/projects/COPY/import", json!({ "toml": text }))
        .await;
    assert!(api.get("/v1/tickets/COPY-1").await["ticket"]["design_first"].is_null());
    assert_eq!(
        api.get("/v1/tickets/COPY-2").await["ticket"]["design_first"],
        true
    );
}

#[tokio::test]
async fn ask_design_on_a_card_that_has_not_started_sets_the_flag_without_a_comment() {
    let api = Api::new();
    api.demo().await;
    let card = api
        .post(
            "/v1/chains/demo/tickets",
            json!({ "title": "A", "paused": true }),
        )
        .await;
    assert_eq!(card["ticket"]["status"], "backlog");
    assert!(offers(&card, "ask_design"));
    let after = api
        .post("/v1/tickets/DEMO-1/actions/ask_design", json!({}))
        .await;
    assert_eq!(after["ticket"]["design_first"], true);
    assert_eq!(after["ticket"]["status"], "backlog");
    assert_eq!(after["comments"], json!([]));
    assert!(!offers(&after, "ask_design"));
    let (s, e) = api
        .call(Method::POST, "/v1/tickets/DEMO-1/actions/ask_design", None)
        .await;
    assert_eq!(s, StatusCode::CONFLICT, "asked twice: {e}");
}

#[tokio::test]
async fn ask_design_on_a_running_card_sends_an_instruction_and_review_refuses_it() {
    let api = Api::new();
    api.demo().await;
    api.post("/v1/chains/demo/tickets", json!({ "title": "A" }))
        .await;
    let running = api.get("/v1/tickets/DEMO-1").await;
    assert_eq!(running["ticket"]["status"], "running");
    assert!(offers(&running, "ask_design"));
    let after = api
        .post("/v1/tickets/DEMO-1/actions/ask_design", json!({}))
        .await;
    assert_eq!(after["ticket"]["design_first"], true);
    let comments = after["comments"].as_array().expect("comments");
    assert_eq!(comments.len(), 1, "{after}");
    assert_eq!(comments[0]["kind"], "instruction");
    assert_eq!(comments[0]["author"], "human");
    assert!(
        comments[0]["body"]["gist"]
            .as_str()
            .expect("gist")
            .starts_with("Before you go further, open a design thread for this story"),
        "{}",
        comments[0]
    );

    api.post(
        "/v1/tickets/DEMO-1/events",
        json!({ "event": "open_thread", "kind": "design", "title": "Screen" }),
    )
    .await;
    api.post(
        "/v1/tickets/DEMO-1/events",
        json!({ "event": "thread_version", "thread": "1", "format": "mermaid", "body": "graph TD; A-->B" }),
    )
    .await;
    let waiting = api.get("/v1/tickets/DEMO-1").await;
    assert_eq!(
        waiting["design_waiting"], true,
        "a version waits for the person"
    );
    api.post("/v1/tickets/DEMO-1/threads/1/approve", json!({}))
        .await;
    assert_eq!(api.get("/v1/tickets/DEMO-1").await["design_waiting"], false);
    api.post(
        "/v1/tickets/DEMO-1/events",
        json!({ "event": "finished", "summary": toknila_core::handin::from_plain("done") }),
    )
    .await;
    let review = api.get("/v1/tickets/DEMO-1").await;
    assert_eq!(review["ticket"]["status"], "review");
    assert!(!offers(&review, "ask_design"));
    let (s, e) = api
        .call(Method::POST, "/v1/tickets/DEMO-1/actions/ask_design", None)
        .await;
    assert_eq!(s, StatusCode::CONFLICT, "{e}");
}

#[tokio::test]
async fn finished_is_refused_on_a_design_first_card_until_a_version_is_approved() {
    let api = Api::new();
    api.demo().await;
    api.post(
        "/v1/chains/demo/tickets",
        json!({ "title": "A", "design_first": true }),
    )
    .await;
    assert_eq!(api.status("DEMO-1").await, "running");
    let finish = || {
        api.call(
            Method::POST,
            "/v1/tickets/DEMO-1/events",
            Some(
                json!({ "event": "finished", "summary": toknila_core::handin::from_plain("done") }),
            ),
        )
    };
    let (s, e) = finish().await;
    assert_eq!(s, StatusCode::CONFLICT, "{e}");
    let msg = e["error"].as_str().expect("error");
    assert!(msg.starts_with("cannot hand in DEMO-1"), "{msg}");
    assert!(
        msg.contains("open_thread") && msg.contains("approve"),
        "{msg}"
    );
    assert_eq!(api.status("DEMO-1").await, "running");

    // An approved thread without a version is not a design a person judged.
    api.post(
        "/v1/tickets/DEMO-1/events",
        json!({ "event": "open_thread", "kind": "flow", "title": "Steps" }),
    )
    .await;
    api.post("/v1/tickets/DEMO-1/threads/1/approve", json!({}))
        .await;
    assert_eq!(finish().await.0, StatusCode::CONFLICT);

    api.post(
        "/v1/tickets/DEMO-1/events",
        json!({ "event": "open_thread", "kind": "design", "title": "Screen" }),
    )
    .await;
    api.post(
        "/v1/tickets/DEMO-1/events",
        json!({ "event": "thread_version", "thread": "2", "format": "mermaid", "body": "graph TD; A-->B" }),
    )
    .await;
    api.post("/v1/tickets/DEMO-1/threads/2/approve", json!({}))
        .await;
    assert_eq!(finish().await.0, StatusCode::OK);
    assert_eq!(api.status("DEMO-1").await, "review");
}
