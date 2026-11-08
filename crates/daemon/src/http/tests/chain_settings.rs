//! `PATCH /chains/{chain}` for autostart through the full router and the real store: turning it
//! on starts the ready card and is journaled, turning it off keeps the chain as it is, and a
//! name two projects share needs `?project=`.

use axum::http::{Method, StatusCode};
use serde_json::json;

use super::Api;

async fn patch(api: &Api, uri: &str, body: serde_json::Value) -> (StatusCode, serde_json::Value) {
    api.call(Method::PATCH, uri, Some(body)).await
}

#[tokio::test]
async fn autostart_is_changed_after_creation_starts_ready_cards_and_is_journaled() {
    let api = Api::new();
    api.post("/v1/projects", json!({ "key": "DEMO", "name": "Demo" }))
        .await;
    api.post(
        "/v1/projects/DEMO/chains",
        json!({ "name": "work", "autostart": false }),
    )
    .await;
    api.post("/v1/chains/work/tickets", json!({ "title": "A" }))
        .await;
    assert_eq!(api.status("DEMO-1").await, "ready", "control: waits");

    let (s, on) = patch(&api, "/v1/chains/work", json!({ "autostart": true })).await;
    assert_eq!(s, StatusCode::OK, "{on}");
    assert_eq!(on["chain"]["autostart"], true);
    assert_eq!(
        api.status("DEMO-1").await,
        "running",
        "the ready card started"
    );
    let events = api.get("/v1/events?since=0").await;
    let updated = events
        .as_array()
        .expect("events")
        .iter()
        .rfind(|e| e["entity"] == "chain" && e["kind"] == "updated")
        .cloned()
        .expect("a chain.updated event");
    assert_eq!(updated["payload"]["chain"]["autostart"], true, "{updated}");

    let (s, off) = patch(&api, "/v1/chains/work", json!({ "autostart": false })).await;
    assert_eq!(s, StatusCode::OK, "{off}");
    assert_eq!(off["chain"]["autostart"], false);
    // A body that leaves the field out keeps it.
    let (_, kept) = patch(&api, "/v1/chains/work", json!({})).await;
    assert_eq!(kept["chain"]["autostart"], false);
}

#[tokio::test]
async fn a_chain_name_two_projects_share_needs_the_project() {
    let api = Api::new();
    for key in ["AAA", "BBB"] {
        api.post("/v1/projects", json!({ "key": key, "name": key }))
            .await;
        api.post(
            &format!("/v1/projects/{key}/chains"),
            json!({ "name": "work", "autostart": false }),
        )
        .await;
    }
    let (s, e) = patch(&api, "/v1/chains/work", json!({ "autostart": true })).await;
    assert_eq!(s, StatusCode::CONFLICT, "{e}");
    let (s, ok) = patch(
        &api,
        "/v1/chains/work?project=BBB",
        json!({ "autostart": true }),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{ok}");
    let chains = api.get("/v1/chains").await;
    let on: Vec<bool> = chains
        .as_array()
        .expect("chains")
        .iter()
        .map(|c| c["chain"]["autostart"].as_bool().expect("flag"))
        .collect();
    assert_eq!(on.iter().filter(|a| **a).count(), 1, "{chains}");
}
