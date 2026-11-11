//! A card that waits goes back to Running when its agent shows life: an MCP event, or a hook that
//! means work. A hook that only says "I stopped" does not.

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};

use super::Api;

async fn status(api: &Api, key: &str) -> Value {
    api.get(&format!("/v1/tickets/{key}")).await["ticket"]["status"].clone()
}

async fn event(api: &Api, token: &str, e: Value) {
    let (s, v) = api
        .call_with(token, Method::POST, "/v1/tickets/DEMO-1/events", Some(e))
        .await;
    assert_eq!(s, StatusCode::OK, "{v}");
}

async fn hook(api: &Api, token: &str, signal: &str) {
    let (s, _) = api
        .call_with(
            token,
            Method::POST,
            "/v1/tickets/DEMO-1/activity",
            Some(json!({ "signal": signal })),
        )
        .await;
    assert_eq!(s, StatusCode::NO_CONTENT);
}

async fn waiting(api: &Api) -> String {
    api.demo().await;
    api.post("/v1/chains/demo/tickets", json!({ "title": "A" }))
        .await;
    let id = api.get("/v1/tickets/DEMO-1").await["ticket"]["id"]
        .as_str()
        .and_then(|s| s.parse().ok())
        .expect("ticket id");
    let grant = crate::runner::Grant {
        ticket: id,
        run: toknila_core::RunId::new(),
        role: toknila_core::AgentRole::Worker,
    };
    let token = api
        .runners
        .tokens()
        .issue(grant, crate::server::now())
        .expect("token");
    event(api, &token, json!({ "event": "started" })).await;
    event(
        api,
        &token,
        json!({ "event": "ask_human", "text": "may I?", "permission": true }),
    )
    .await;
    assert_eq!(status(api, "DEMO-1").await, "waiting");
    token
}

#[tokio::test]
async fn a_hook_that_means_work_returns_a_waiting_card_to_running() {
    let api = Api::new();
    let token = waiting(&api).await;
    hook(&api, &token, "hook:stop").await;
    hook(&api, &token, "hook:notification").await;
    assert_eq!(status(&api, "DEMO-1").await, "waiting");
    hook(&api, &token, "hook:post_tool_use").await;
    assert_eq!(status(&api, "DEMO-1").await, "running");
}

#[tokio::test]
async fn a_hook_of_work_never_clears_an_open_question_and_the_answer_still_does() {
    let api = Api::new();
    api.demo().await;
    api.post(
        "/v1/chains/demo/tickets",
        json!({ "title": "A", "profile": "sim" }),
    )
    .await;
    let (token, _) = api.agent_token("DEMO-1").await;
    event(&api, &token, json!({ "event": "started" })).await;
    event(
        &api,
        &token,
        json!({ "event": "ask_human", "text": "which option?" }),
    )
    .await;
    let inbox = || async {
        api.get("/v1/projects/DEMO/inbox")
            .await
            .as_array()
            .expect("items")
            .iter()
            .filter(|i| i["kind"] == "question")
            .cloned()
            .collect::<Vec<_>>()
    };
    assert_eq!(inbox().await.len(), 1);
    // A nudge typed into the agent fires these two; neither is an answer.
    hook(&api, &token, "hook:user_prompt_submit").await;
    hook(&api, &token, "hook:post_tool_use").await;
    assert_eq!(status(&api, "DEMO-1").await, "waiting");
    let q = inbox().await;
    assert_eq!(q.len(), 1, "{q:?}");
    assert!(
        q[0]["text"]
            .as_str()
            .is_some_and(|t| t.contains("which option?"))
    );
    // The answer reaches the agent and returns the card to Running.
    let t = api
        .post("/v1/tickets/DEMO-1/comments", json!({ "text": "option b" }))
        .await;
    assert!(t["comments"][1]["delivered_at"].is_number(), "{t}");
    assert_eq!(status(&api, "DEMO-1").await, "running");
    assert!(inbox().await.is_empty());
}

#[tokio::test]
async fn an_mcp_event_leaves_a_waiting_card_where_it_is() {
    let api = Api::new();
    let token = waiting(&api).await;
    event(
        &api,
        &token,
        json!({ "event": "note", "body": { "gist": "continuing" } }),
    )
    .await;
    event(&api, &token, json!({ "event": "started" })).await;
    assert_eq!(status(&api, "DEMO-1").await, "waiting");
}

#[tokio::test]
async fn a_reviewers_activity_never_resumes_a_waiting_worker() {
    let api = Api::new();
    let _worker = waiting(&api).await;
    let id = api.get("/v1/tickets/DEMO-1").await["ticket"]["id"]
        .as_str()
        .and_then(|s| s.parse().ok())
        .expect("ticket id");
    let grant = crate::runner::Grant {
        ticket: id,
        run: toknila_core::RunId::new(),
        role: toknila_core::AgentRole::Reviewer,
    };
    let reviewer = api
        .runners
        .tokens()
        .issue(grant, crate::server::now())
        .expect("token");
    hook(&api, &reviewer, "hook:post_tool_use").await;
    assert_eq!(status(&api, "DEMO-1").await, "waiting");
}
