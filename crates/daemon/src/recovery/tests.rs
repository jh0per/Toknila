use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::{Value, json};
use tower::ServiceExt;

use super::*;
use crate::server::{AppState, router};

async fn post(app: &axum::Router, uri: &str, body: Value) {
    let req = Request::builder()
        .method("POST")
        .uri(uri)
        .header("authorization", "Bearer t")
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .expect("request");
    let resp = app.clone().oneshot(req).await.expect("response");
    assert!(
        resp.status().is_success() || resp.status() == StatusCode::CREATED,
        "{uri}: {}",
        resp.status()
    );
}

async fn seeded() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = Store::open(&dir.path().join("t.db")).expect("store");
    let app = router(AppState::for_test(store.clone(), "t", dir.path()));
    post(&app, "/v1/projects", json!({ "key": "DEMO" })).await;
    let toml = include_str!("../../../../examples/demo.toml");
    post(&app, "/v1/projects/DEMO/import", json!({ "toml": toml })).await;
    post(
        &app,
        "/v1/tickets/DEMO-1/events",
        json!({ "event": "finished", "summary": toknila_core::handin::from_plain("done") }),
    )
    .await;
    post(&app, "/v1/tickets/DEMO-1/actions/accept", json!({})).await;
    store.settled().await;
    (dir, store)
}

#[tokio::test]
async fn a_clean_store_has_no_drift_and_keeps_sim_tickets_running() {
    let (_dir, store) = seeded().await;
    let r = recover(&store).await.expect("recover");
    assert_eq!(
        r,
        Recovery {
            chains: 1,
            tickets: 4,
            running: 2,
            lost: vec![],
            reviews_lost: vec![],
            merges_resumed: vec![],
            drift: vec![]
        }
    );
    // Recovery of a settled store writes nothing.
    let before = store.read(|repo| repo.last_seq()).await.expect("seq");
    recover(&store).await.expect("again");
    assert_eq!(
        store.read(|repo| repo.last_seq()).await.expect("seq"),
        before
    );
}

#[tokio::test]
async fn state_changed_behind_the_journal_is_reported() {
    let (dir, store) = seeded().await;
    let raw = rusqlite::Connection::open(dir.path().join("t.db")).expect("raw");
    let n = raw
        .execute(
            "UPDATE tickets SET title = 'tampered' WHERE key = 'DEMO-3'",
            [],
        )
        .expect("update");
    assert_eq!(n, 1, "the probe must hit a row");
    let r = recover(&store).await.expect("recover");
    assert_eq!(r.drift, ["DEMO-3"]);
}
