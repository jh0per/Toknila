//! The project's setting for commands and "Allow always", through the full router: the setting
//! is saved and survives a restart, an answer with `always` remembers a rule, and a prompt the
//! rule covers is answered without asking while one it does not cover still waits for a person.

use std::time::Duration;

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};

use super::Api;

const PROJECT: &str = "/v1/projects/DEMO";

async fn setup() -> (Api, String) {
    let api = Api::new();
    api.demo_with(&["A"]).await;
    let (token, _) = api.agent_token("DEMO-1").await;
    api.call_with(
        &token,
        Method::POST,
        "/v1/tickets/DEMO-1/events",
        Some(json!({ "event": "started" })),
    )
    .await;
    (api, token)
}

async fn permission(api: &Api, token: &str, tool: &str) -> (StatusCode, Value) {
    api.call_with(
        token,
        Method::POST,
        "/v1/tickets/DEMO-1/permission",
        Some(json!({ "tool": tool })),
    )
    .await
}

async fn answer(api: &Api, body: Value) -> (StatusCode, Value) {
    api.call(Method::POST, "/v1/tickets/DEMO-1/runner/answer", Some(body))
        .await
}

async fn patch(api: &Api, body: Value) -> (StatusCode, Value) {
    api.call(Method::PATCH, PROJECT, Some(body)).await
}

/// Waits until the worker's prompt is open.
async fn wait_for_prompt(api: &Api) {
    for _ in 0..200 {
        let inbox = api.get("/v1/projects/DEMO/inbox").await;
        if inbox.as_array().is_some_and(|items| !items.is_empty()) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("the prompt never reached the inbox");
}

#[tokio::test]
async fn the_commands_setting_is_new_projects_auto_and_old_ones_ask_and_it_survives_a_restart() {
    let api = Api::new();
    api.post("/v1/projects", json!({ "key": "DEMO" })).await;
    api.post(
        "/v1/projects",
        json!({ "key": "OLD", "command_policy": "ask" }),
    )
    .await;
    assert_eq!(api.get(PROJECT).await["command_policy"], "auto");
    assert_eq!(api.get("/v1/projects/OLD").await["command_policy"], "ask");

    let (s, p) = patch(&api, json!({ "command_policy": "trust" })).await;
    assert_eq!((s, &p["command_policy"]), (StatusCode::OK, &json!("trust")));
    let (s, e) = patch(&api, json!({ "command_policy": "yolo" })).await;
    assert!(s.is_client_error(), "{e}");

    let api = api.restart();
    assert_eq!(api.get(PROJECT).await["command_policy"], "trust");
}

#[tokio::test]
async fn allow_always_remembers_a_rule_and_a_covered_prompt_is_answered_without_asking() {
    let (api, token) = setup().await;
    assert!(api.get(PROJECT).await.get("allowed_commands").is_none());

    // The first prompt waits for the person, who allows it always.
    let hook = permission(&api, &token, "Bash: cd \"/w/ship\"; cargo test -p core");
    let person = async {
        wait_for_prompt(&api).await;
        let (s, v) = answer(&api, json!({ "answer": "allow", "always": true })).await;
        assert_eq!(s, StatusCode::NO_CONTENT, "{v}");
    };
    let ((s, reply), ()) = tokio::join!(hook, person);
    assert_eq!((s, reply), (StatusCode::OK, json!({ "decision": "allow" })));
    assert_eq!(
        api.get(PROJECT).await["allowed_commands"],
        json!(["Bash(cargo test *)"])
    );

    // A command the rule covers is allowed at once: no prompt, the card keeps running.
    let (s, reply) = permission(&api, &token, "Bash: cargo test --lib").await;
    assert_eq!((s, reply), (StatusCode::OK, json!({ "decision": "allow" })));
    assert_eq!(api.get("/v1/projects/DEMO/inbox").await, json!([]));
    assert_eq!(api.status("DEMO-1").await, "running");

    // Control: the same words in a list of commands, another command and another tool are asked.
    for tool in [
        "Bash: cargo test; rm -rf /",
        "Bash: cargo publish",
        "Write: src/lib.rs",
    ] {
        let hook = permission(&api, &token, tool);
        let person = async {
            wait_for_prompt(&api).await;
            let (s, v) = answer(&api, json!({ "answer": "deny" })).await;
            assert_eq!(s, StatusCode::NO_CONTENT, "{v}");
        };
        let ((s, reply), ()) = tokio::join!(hook, person);
        assert_eq!(s, StatusCode::OK);
        assert_eq!(reply["decision"], "deny", "{tool}: {reply}");
    }
}

#[tokio::test]
async fn allow_always_without_a_bash_call_allows_and_remembers_nothing() {
    let (api, token) = setup().await;
    let hook = permission(&api, &token, "WebFetch: https://example.com");
    let person = async {
        wait_for_prompt(&api).await;
        let (s, v) = answer(&api, json!({ "answer": "allow", "always": true })).await;
        assert_eq!(s, StatusCode::NO_CONTENT, "{v}");
    };
    let ((_, reply), ()) = tokio::join!(hook, person);
    assert_eq!(reply, json!({ "decision": "allow" }));
    assert!(api.get(PROJECT).await.get("allowed_commands").is_none());
}

#[tokio::test]
async fn remembered_rules_are_replaced_as_a_list_and_a_bad_rule_is_refused_at_its_field() {
    let api = Api::new();
    api.post("/v1/projects", json!({ "key": "DEMO" })).await;
    let (s, p) = patch(
        &api,
        json!({ "allowed_commands": ["Bash(ls *)", "Bash(make check *)", "Bash(ls *)"] }),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(
        p["allowed_commands"],
        json!(["Bash(ls *)", "Bash(make check *)"])
    );

    let (s, e) = patch(&api, json!({ "allowed_commands": ["rm -rf /"] })).await;
    assert_eq!(
        (s, e["field"].as_str()),
        (StatusCode::BAD_REQUEST, Some("allowed_commands")),
        "{e}"
    );
    assert_eq!(
        api.get(PROJECT).await["allowed_commands"]
            .as_array()
            .map(Vec::len),
        Some(2)
    );

    let api = api.restart();
    assert_eq!(
        api.get(PROJECT).await["allowed_commands"][1],
        "Bash(make check *)"
    );
    let (s, p) = patch(&api, json!({ "allowed_commands": ["Bash(ls *)"] })).await;
    assert_eq!(
        (s, p["allowed_commands"].as_array().map(Vec::len)),
        (StatusCode::OK, Some(1))
    );
    let (_, p) = patch(&api, json!({ "allowed_commands": [] })).await;
    assert!(
        p.get("allowed_commands").is_none(),
        "the last one removed: {p}"
    );
}
