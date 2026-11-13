//! Planning from the window through the router: appending cards, editing, moving, archiving, and
//! the suggestions of the folder check.

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};

use super::Api;

/// The keys of a chain's cards in dependency order.
async fn order(api: &Api, chain: &str) -> Vec<String> {
    let detail = api.get(&format!("/v1/chains/{chain}")).await;
    let key_of = |id: &Value| {
        detail["tickets"]
            .as_array()
            .expect("tickets")
            .iter()
            .find(|t| t["id"] == *id)
            .map(|t| t["key"].as_str().expect("key").to_owned())
            .expect("ticket of the order")
    };
    detail["order"]
        .as_array()
        .expect("order")
        .iter()
        .map(|n| key_of(&n["id"]))
        .collect()
}

async fn manual(api: &Api, cards: usize) {
    api.post("/v1/projects", json!({ "key": "DEMO", "name": "Demo" }))
        .await;
    api.post(
        "/v1/projects/DEMO/chains",
        json!({ "name": "plan", "autostart": false }),
    )
    .await;
    for i in 1..=cards {
        api.post(
            "/v1/chains/plan/tickets",
            json!({ "title": format!("card {i}"), "at_end": true }),
        )
        .await;
    }
}

#[tokio::test]
async fn cards_added_at_the_end_form_a_line_and_one_without_it_stands_alone() {
    let api = Api::new();
    manual(&api, 3).await;
    assert_eq!(order(&api, "plan").await, ["DEMO-1", "DEMO-2", "DEMO-3"]);
    let detail = api.get("/v1/chains/plan").await;
    assert_eq!(detail["edges"].as_array().map(Vec::len), Some(2));
    api.post("/v1/chains/plan/tickets", json!({ "title": "alone" }))
        .await;
    let detail = api.get("/v1/chains/plan").await;
    assert_eq!(detail["edges"].as_array().map(Vec::len), Some(2));
}

#[tokio::test]
async fn a_card_made_before_another_becomes_its_dependency_or_nothing_is_made() {
    let api = Api::new();
    manual(&api, 2).await;
    let made = api
        .post(
            "/v1/chains/plan/tickets",
            json!({ "title": "first", "before": ["DEMO-1"] }),
        )
        .await;
    assert_eq!(made["ticket"]["key"], "DEMO-3");
    assert_eq!(order(&api, "plan").await, ["DEMO-3", "DEMO-1", "DEMO-2"]);
    api.post("/v1/tickets/DEMO-3/actions/start", json!({}))
        .await;
    let (s, e) = api
        .call(
            Method::POST,
            "/v1/chains/plan/tickets",
            Some(json!({ "title": "too late", "before": ["DEMO-3"] })),
        )
        .await;
    assert_eq!(s, StatusCode::CONFLICT, "{e}");
    let detail = api.get("/v1/chains/plan").await;
    assert_eq!(detail["tickets"].as_array().map(Vec::len), Some(3));
}

#[tokio::test]
async fn a_card_moves_and_the_order_follows() {
    let api = Api::new();
    manual(&api, 3).await;
    let moved = api
        .post(
            "/v1/chains/plan/tickets/DEMO-3/move",
            json!({ "anchor": "DEMO-1", "place": "before" }),
        )
        .await;
    assert_eq!(moved["chain"]["name"], "plan");
    assert_eq!(order(&api, "plan").await, ["DEMO-3", "DEMO-1", "DEMO-2"]);
    api.post(
        "/v1/chains/plan/tickets/DEMO-3/move",
        json!({ "anchor": "DEMO-1", "place": "after" }),
    )
    .await;
    assert_eq!(order(&api, "plan").await, ["DEMO-1", "DEMO-3", "DEMO-2"]);
}

#[tokio::test]
async fn a_card_that_started_cannot_move_and_cannot_be_put_before_one_that_did() {
    let api = Api::new();
    manual(&api, 3).await;
    api.post("/v1/tickets/DEMO-1/actions/start", json!({}))
        .await;
    for (card, anchor, place) in [
        ("DEMO-1", "DEMO-3", "after"),
        ("DEMO-3", "DEMO-1", "before"),
    ] {
        let (s, e) = api
            .call(
                Method::POST,
                &format!("/v1/chains/plan/tickets/{card}/move"),
                Some(json!({ "anchor": anchor, "place": place })),
            )
            .await;
        assert_eq!(s, StatusCode::CONFLICT, "{card} {place} {anchor}: {e}");
    }
    assert_eq!(order(&api, "plan").await, ["DEMO-1", "DEMO-2", "DEMO-3"]);
    let detail = api.get("/v1/tickets/DEMO-1").await;
    assert_eq!(detail["can_move"], false);
    assert_eq!(api.get("/v1/tickets/DEMO-2").await["can_move"], true);
}

#[tokio::test]
async fn a_card_is_edited_until_it_is_finished() {
    let api = Api::new();
    manual(&api, 2).await;
    let (s, d) = api
        .call(
            Method::PATCH,
            "/v1/tickets/DEMO-1",
            Some(json!({ "title": "renamed", "prompt": "## Do it\n\nwell" })),
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{d}");
    assert_eq!(d["ticket"]["title"], "renamed");
    assert_eq!(d["ticket"]["prompt"], "## Do it\n\nwell");
    assert_eq!(d["can_edit"], true);
    let (s, _) = api
        .call(
            Method::PATCH,
            "/v1/tickets/DEMO-1",
            Some(json!({ "title": "  " })),
        )
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);

    api.post(
        "/v1/tickets/DEMO-2/actions/cancel",
        json!({ "text": "not needed" }),
    )
    .await;
    let (s, e) = api
        .call(
            Method::PATCH,
            "/v1/tickets/DEMO-2",
            Some(json!({ "title": "too late" })),
        )
        .await;
    assert_eq!(
        (s, e["code"].as_str()),
        (StatusCode::CONFLICT, Some("conflict"))
    );
    assert_eq!(api.get("/v1/tickets/DEMO-2").await["can_edit"], false);
    assert_eq!(
        api.get("/v1/tickets/DEMO-2").await["ticket"]["title"],
        "card 2"
    );
}

#[tokio::test]
async fn a_project_is_archived_and_brought_back_with_its_data() {
    let api = Api::new();
    manual(&api, 1).await;
    let p = api.post("/v1/projects/DEMO/archive", json!({})).await;
    assert_eq!(p["archived"], true);
    let all = api.get("/v1/projects").await;
    assert_eq!(all[0]["archived"], true);
    assert_eq!(order(&api, "plan").await, ["DEMO-1"]);
    let p = api.post("/v1/projects/DEMO/unarchive", json!({})).await;
    assert!(p.get("archived").is_none(), "{p}");
}

#[tokio::test]
async fn archiving_is_refused_while_an_agent_of_the_project_works() {
    let api = Api::new();
    manual(&api, 1).await;
    api.post("/v1/tickets/DEMO-1/actions/start", json!({}))
        .await;
    let (s, e) = api
        .call(Method::POST, "/v1/projects/DEMO/archive", Some(json!({})))
        .await;
    assert_eq!(s, StatusCode::CONFLICT, "{e}");
    assert!(
        e["error"].as_str().is_some_and(|m| m.contains("agent")),
        "{e}"
    );
    assert!(api.get("/v1/projects/DEMO").await.get("archived").is_none());
}

#[tokio::test]
async fn a_project_is_hidden_and_unhidden_and_the_list_says_so() {
    let api = Api::new();
    manual(&api, 1).await;
    let p = api.post("/v1/projects/DEMO/hide", json!({})).await;
    assert_eq!(p["hidden"], true);
    assert!(p.get("archived").is_none(), "{p}");
    assert_eq!(api.get("/v1/projects").await[0]["hidden"], true);
    let again = api.post("/v1/projects/DEMO/hide", json!({})).await;
    assert_eq!(again, p);
    let p = api.post("/v1/projects/DEMO/unhide", json!({})).await;
    assert!(p.get("hidden").is_none(), "{p}");
    let again = api.post("/v1/projects/DEMO/unhide", json!({})).await;
    assert_eq!(again, p);
    assert!(api.get("/v1/projects").await[0].get("hidden").is_none());
    for route in ["hide", "unhide"] {
        let (s, e) = api
            .call(
                Method::POST,
                &format!("/v1/projects/NOPE/{route}"),
                Some(json!({})),
            )
            .await;
        assert_eq!(s, StatusCode::NOT_FOUND, "{route}: {e}");
    }
}

#[tokio::test]
async fn hiding_is_accepted_while_an_agent_of_the_project_works_and_changes_nothing_else() {
    let api = Api::new();
    manual(&api, 1).await;
    api.post("/v1/tickets/DEMO-1/actions/start", json!({}))
        .await;
    let status = api.get("/v1/tickets/DEMO-1").await["ticket"]["status"].clone();
    assert_eq!(status, "running");
    let inbox = api.get("/v1/projects/DEMO/inbox").await;
    let p = api.post("/v1/projects/DEMO/hide", json!({})).await;
    assert_eq!(p["hidden"], true);
    assert_eq!(
        api.get("/v1/tickets/DEMO-1").await["ticket"]["status"],
        status
    );
    assert_eq!(api.get("/v1/projects/DEMO/inbox").await, inbox);
}

#[tokio::test]
async fn the_folder_check_suggests_a_key_and_a_name() {
    let api = Api::new();
    let dir = tempfile::tempdir().expect("tempdir");
    let folder = dir.path().join("Toknila Notes");
    std::fs::create_dir(&folder).expect("folder");
    let path = folder.to_str().expect("utf-8 path");
    let check = api
        .post("/v1/repos/check", json!({ "repo_path": path }))
        .await;
    assert_eq!(check["suggested_key"], "TOKNILANOT");
    assert_eq!(check["suggested_name"], "Toknila Notes");
}
