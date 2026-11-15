//! The workspaces list and Clean up of finished stories, through the router on a temporary git
//! repository (TOKNILA-132).

use std::sync::Arc;

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};
use toknila_core::{Actor, Ship};

use super::Api;
use super::ships::{project, repo};
use crate::server::now;
use crate::shipyard::GitShipyard;

#[tokio::test]
async fn the_list_marks_finished_stories_names_the_planner_copy_and_clean_up_retires_them() {
    let tmp = tempfile::tempdir().expect("tmp");
    let repo = repo(tmp.path());
    let api = Api::with_yard(Some(Arc::new(GitShipyard::new(tmp.path().join("ships")))));
    project(&api, &repo).await;
    api.post("/v1/chains/auth/tickets", json!({ "title": "Login" }))
        .await;
    let planner = api._dir.path().join("ships").join("demo").join("_planner");
    std::fs::create_dir_all(&planner).expect("planner folder");

    let list = api.get("/v1/projects/DEMO/ships").await;
    assert_eq!(
        (list["count"].as_u64(), list["finished"].as_u64()),
        (Some(1), Some(0))
    );
    let items = list["items"].as_array().expect("items");
    assert_eq!(
        items[0]["kind"], "planner",
        "the assistant's copy is named: {list}"
    );
    assert_eq!(items[0]["finished"], false);
    assert_eq!(items[1]["story"], "DEMO-1");
    assert_eq!(items[1]["status"], "running");

    // The story is cancelled and retires its ship; an old daemon would have left the record.
    let before = api.get("/v1/tickets/DEMO-1").await;
    let ship: Ship = serde_json::from_value(before["ship"].clone()).expect("a ship");
    api.post(
        "/v1/tickets/DEMO-1/actions/cancel",
        json!({ "text": "not needed" }),
    )
    .await;
    assert_eq!(api.get("/v1/tickets/DEMO-1").await["ship"], Value::Null);
    let planted = ship.clone();
    api.store
        .write(
            Actor::System,
            now(),
            move |tx| -> Result<(), crate::store::StoreError> {
                use toknila_app::UnitOfWork;
                tx.save_ship(&planted)
                    .map_err(|e| crate::store::StoreError::Corrupt(e.to_string()))
            },
        )
        .await
        .expect("plant the record");

    let list = api.get("/v1/projects/DEMO/ships").await;
    assert_eq!(
        (list["count"].as_u64(), list["finished"].as_u64()),
        (Some(1), Some(1))
    );
    let row = &list["items"].as_array().expect("items")[1];
    assert_eq!(
        (row["status"].as_str(), row["finished"].as_bool()),
        (Some("cancelled"), Some(true))
    );

    let done = api
        .post("/v1/projects/DEMO/ships/retire-finished", json!({}))
        .await;
    assert_eq!(done["retired"], json!(["DEMO-1"]));
    let list = api.get("/v1/projects/DEMO/ships").await;
    assert_eq!(
        (list["count"].as_u64(), list["finished"].as_u64()),
        (Some(0), Some(0))
    );
    assert_eq!(
        list["items"][0]["kind"], "planner",
        "the copy is never retired"
    );
    assert!(planner.is_dir());
}

#[tokio::test]
async fn an_agent_token_cannot_retire_workspaces() {
    let api = Api::new();
    api.demo_with(&["A"]).await;
    let (token, _) = api.agent_token("DEMO-1").await;
    let (status, body) = api
        .call_with(
            &token,
            Method::POST,
            "/v1/projects/DEMO/ships/retire-finished",
            Some(json!({})),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
}
