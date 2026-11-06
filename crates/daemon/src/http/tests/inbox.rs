//! The inbox route and the leftover decisions through the full router and the real store.

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};

use super::Api;

/// `[(kind, ticket key)]` of the project's inbox, in the order the daemon answers.
async fn inbox(api: &Api) -> Vec<(String, String)> {
    let items = api.get("/v1/projects/DEMO/inbox").await;
    items
        .as_array()
        .expect("a list")
        .iter()
        .map(|i| {
            (
                i["kind"].as_str().expect("kind").to_owned(),
                i["ticket"].as_str().expect("ticket").to_owned(),
            )
        })
        .collect()
}

#[tokio::test]
async fn the_inbox_lists_a_proposal_a_question_and_a_blocked_card_until_they_are_answered() {
    let api = Api::new();
    api.demo().await;
    for title in ["asks", "blocks"] {
        api.post(
            "/v1/chains/demo/tickets",
            json!({ "title": title, "profile": "sim" }),
        )
        .await;
    }
    api.post(
        "/v1/chains/demo/wildcards",
        json!({ "title": "found work", "prompt": "do it" }),
    )
    .await;
    api.post(
        "/v1/tickets/DEMO-1/events",
        json!({ "event": "ask_human", "text": "which framework?" }),
    )
    .await;
    api.post(
        "/v1/tickets/DEMO-2/actions/block",
        json!({ "text": "waiting for the key" }),
    )
    .await;

    let items = api.get("/v1/projects/DEMO/inbox").await;
    let by_kind = |kind: &str| -> Value {
        items
            .as_array()
            .expect("list")
            .iter()
            .find(|i| i["kind"] == kind)
            .cloned()
            .unwrap_or(Value::Null)
    };
    assert_eq!(items.as_array().expect("list").len(), 3, "{items}");
    assert_eq!(by_kind("question")["ticket"], "DEMO-1");
    assert_eq!(by_kind("question")["text"], "which framework?");
    assert_eq!(by_kind("blocked")["ticket"], "DEMO-2");
    assert_eq!(by_kind("blocked")["text"], "manual: waiting for the key");
    assert_eq!(by_kind("proposal")["ticket"], "DEMO-3");
    assert_eq!(by_kind("proposal")["title"], "found work");

    // The reply is an instruction comment; delivery answers the question.
    api.post(
        "/v1/tickets/DEMO-1/comments",
        json!({ "kind": "instruction", "text": "use axum" }),
    )
    .await;
    api.post("/v1/tickets/DEMO-2/actions/unblock", json!({}))
        .await;
    api.post("/v1/tickets/DEMO-3/actions/approve", json!({}))
        .await;
    assert_eq!(inbox(&api).await, []);
}

#[tokio::test]
async fn a_project_that_does_not_exist_has_no_inbox() {
    let api = Api::new();
    let (status, body) = api.call(Method::GET, "/v1/projects/NOPE/inbox", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
}

const FILE: &str = r#"
[chain]
name = "imported"
autostart = false

[[ticket]]
id = "A"
subtasks = ["one", "two"]
done_subtasks = [1]

[[ticket]]
id = "B"
done = true
after = ["A"]
"#;

async fn imported(api: &Api) {
    api.demo().await;
    api.post("/v1/projects/DEMO/import", json!({ "toml": FILE }))
        .await;
}

#[tokio::test]
async fn leftovers_are_decided_once_and_the_inbox_follows() {
    let api = Api::new();
    imported(&api).await;
    // The import's cards are DEMO-1 (A, open) and DEMO-2 (B, done behind it).
    assert_eq!(
        inbox(&api).await,
        [("leftovers".to_owned(), "DEMO-1".to_owned())]
    );
    let item = &api.get("/v1/projects/DEMO/inbox").await[0];
    assert_eq!(item["subtasks"], json!(["two"]));
    assert_eq!(item["done_cards"], json!(["DEMO-2"]));

    let d = api
        .post("/v1/tickets/DEMO-1/leftovers/move", json!({}))
        .await;
    assert_eq!(d["subtasks"].as_array().expect("subtasks").len(), 1);
    assert_eq!(d["subtasks"][0]["text"], "one");
    let moved = api.get("/v1/tickets/DEMO-3").await;
    assert_eq!(moved["ticket"]["title"], "Leftovers from A");
    assert_eq!(moved["subtasks"][0]["text"], "two");
    assert_eq!(inbox(&api).await, []);

    let (status, body) = api
        .call(
            Method::POST,
            "/v1/tickets/DEMO-1/leftovers/close",
            Some(json!({})),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    let (status, _) = api
        .call(
            Method::POST,
            "/v1/tickets/DEMO-1/leftovers/shred",
            Some(json!({})),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn leaving_leftovers_survives_a_restart_of_the_view() {
    let api = Api::new();
    imported(&api).await;
    api.post("/v1/tickets/DEMO-1/leftovers/leave", json!({}))
        .await;
    assert_eq!(inbox(&api).await, []);
    let marked = api.get("/v1/tickets/DEMO-2").await;
    assert_eq!(
        marked["ticket"]["done_out_of_order"]["dismissed"],
        json!(["DEMO-1"])
    );
}

#[tokio::test]
async fn closing_leftovers_marks_the_subtasks_done() {
    let api = Api::new();
    imported(&api).await;
    let d = api
        .post("/v1/tickets/DEMO-1/leftovers/close", json!({}))
        .await;
    assert!(
        d["subtasks"]
            .as_array()
            .expect("subtasks")
            .iter()
            .all(|s| s["done"] == true),
        "{d}"
    );
    assert_eq!(d["comments"][0]["kind"], "decision");
    assert_eq!(inbox(&api).await, []);
}
