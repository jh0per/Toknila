//! An epic's release waits for every story of the epic, whichever way a story gets there: the
//! window's New story, the assistant's add, a story moved in, and one moved out.

use axum::http::Method;
use serde_json::{Value, json};

use super::Api;

/// Project DEMO with the manual epic `auth` (release 0.3.0, no stories) and the epic `src`
/// holding one story, DEMO-1.
async fn setup() -> Api {
    let api = Api::new();
    api.post("/v1/projects", json!({ "key": "DEMO", "name": "Demo" }))
        .await;
    api.post(
        "/v1/projects/DEMO/chains",
        json!({ "name": "src", "autostart": false }),
    )
    .await;
    api.post("/v1/chains/src/tickets", json!({ "title": "away" }))
        .await;
    api.post(
        "/v1/projects/DEMO/chains",
        json!({ "name": "auth", "autostart": false, "version": "0.3.0" }),
    )
    .await;
    api
}

/// The keys the epic's release waits for, sorted.
async fn release_waits_for(api: &Api) -> Vec<String> {
    let d = api.get("/v1/chains/auth").await;
    let release = d["releases"][0]["id"].as_str().expect("release id");
    let key = |id: &Value| {
        d["tickets"]
            .as_array()
            .expect("tickets")
            .iter()
            .find(|t| t["id"] == *id)
            .map(|t| t["key"].as_str().expect("key").to_owned())
            .expect("a ticket of the epic")
    };
    let mut keys: Vec<String> = d["edges"]
        .as_array()
        .expect("edges")
        .iter()
        .filter(|e| e["to"]["id"] == release)
        .map(|e| key(&e["from"]["id"]))
        .collect();
    keys.sort();
    keys
}

#[tokio::test]
async fn a_story_added_by_the_window_or_the_assistant_goes_before_the_release() {
    let api = setup().await;
    assert_eq!(release_waits_for(&api).await, Vec::<String>::new());

    // The window's New story.
    api.post("/v1/chains/auth/tickets", json!({ "title": "window" }))
        .await;
    assert_eq!(release_waits_for(&api).await, ["DEMO-2"]);

    // The assistant's add: a planner token, the planner route.
    let token = api.planner_token("DEMO").await;
    let (s, card) = api
        .call_with(
            &token,
            Method::POST,
            "/v1/planner/chains/auth/cards",
            Some(json!({ "title": "assistant", "after": ["DEMO-2"] })),
        )
        .await;
    assert_eq!(s, axum::http::StatusCode::CREATED, "{card}");
    // DEMO-3 waits for DEMO-2: only the leaf is before the release.
    assert_eq!(release_waits_for(&api).await, ["DEMO-3"]);
}

#[tokio::test]
async fn a_story_moved_in_goes_before_the_release_and_one_moved_out_leaves_it() {
    let api = setup().await;
    api.post("/v1/chains/auth/tickets", json!({ "title": "own" }))
        .await;
    assert_eq!(release_waits_for(&api).await, ["DEMO-2"]);

    let (s, v) = api
        .call(
            Method::POST,
            "/v1/tickets/DEMO-1/move-to-chain",
            Some(json!({ "chain": "auth" })),
        )
        .await;
    assert_eq!(s, axum::http::StatusCode::OK, "{v}");
    // A moved story joins at the tail (after DEMO-2), so it is the one leaf the release waits for.
    assert_eq!(release_waits_for(&api).await, ["DEMO-1"]);

    // Detach it (a story with dependencies cannot be moved), then move it out.
    let (s, v) = api
        .call(
            Method::DELETE,
            "/v1/chains/auth/edges?from=DEMO-2&to=DEMO-1",
            None,
        )
        .await;
    assert_eq!(s, axum::http::StatusCode::OK, "{v}");
    assert_eq!(release_waits_for(&api).await, ["DEMO-1", "DEMO-2"]);
    let (s, v) = api
        .call(
            Method::POST,
            "/v1/tickets/DEMO-1/move-to-chain",
            Some(json!({ "chain": "src" })),
        )
        .await;
    assert_eq!(s, axum::http::StatusCode::OK, "{v}");
    assert_eq!(release_waits_for(&api).await, ["DEMO-2"]);
}
