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
    post(
        &app,
        "/v1/projects",
        json!({ "key": "DEMO", "default_profile": "sim", "simulated": true }),
    )
    .await;
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
            checks_resumed: vec![],
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

/// Appends journal events with no snapshot after the last one of an entity, as the daemon does.
fn append_bare_events(dir: &tempfile::TempDir, entity: &str, id: &str, kinds: &[&str]) {
    let raw = rusqlite::Connection::open(dir.path().join("t.db")).expect("raw");
    for kind in kinds {
        let n = raw
            .execute(
                "INSERT INTO events (at, actor, entity, entity_id, kind, payload_json) \
                 VALUES (1, 'system', ?1, ?2, ?3, '{}')",
                rusqlite::params![entity, id, kind],
            )
            .expect("insert");
        assert_eq!(n, 1);
    }
}

fn one_id(dir: &tempfile::TempDir, sql: &str) -> String {
    let raw = rusqlite::Connection::open(dir.path().join("t.db")).expect("raw");
    raw.query_row(sql, [], |r| r.get(0)).expect("id")
}

#[tokio::test]
async fn a_merged_card_whose_last_events_carry_no_snapshot_has_no_drift() {
    let (dir, store) = seeded().await;
    let id = one_id(&dir, "SELECT id FROM tickets WHERE key = 'DEMO-1'");
    // DEMO-1 was accepted (status_changed); the real tail is ship_removed, runner_exited.
    append_bare_events(&dir, "ticket", &id, &["ship_removed", "runner_exited"]);
    let r = recover(&store).await.expect("recover");
    assert_eq!(r.drift, Vec::<String>::new());
}

#[tokio::test]
async fn a_tampered_row_still_drifts_behind_snapshotless_events() {
    let (dir, store) = seeded().await;
    let id = one_id(&dir, "SELECT id FROM tickets WHERE key = 'DEMO-1'");
    append_bare_events(&dir, "ticket", &id, &["ship_removed", "runner_exited"]);
    let raw = rusqlite::Connection::open(dir.path().join("t.db")).expect("raw");
    let n = raw
        .execute(
            "UPDATE tickets SET title = 'tampered' WHERE key = 'DEMO-1'",
            [],
        )
        .expect("update");
    assert_eq!(n, 1);
    let r = recover(&store).await.expect("recover");
    assert_eq!(r.drift, ["DEMO-1"]);
}

#[tokio::test]
async fn a_running_card_with_no_run_record_is_lost_on_a_real_project() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = Store::open(&dir.path().join("t.db")).expect("store");
    let app = router(AppState::for_test(store.clone(), "t", dir.path()));
    post(
        &app,
        "/v1/projects",
        json!({ "key": "REAL", "default_profile": "claude-code" }),
    )
    .await;
    // The card names `sim` itself, as a chain file may: it starts no process and leaves no run.
    let toml = "[chain]\nname = \"c\"\ngoal = \"g\"\n[[ticket]]\nid = \"A\"\ntitle = \"A\"\nprofile = \"sim\"\n";
    post(&app, "/v1/projects/REAL/import", json!({ "toml": toml })).await;
    store.settled().await;
    let status = || {
        let raw = rusqlite::Connection::open(dir.path().join("t.db")).expect("raw");
        raw.query_row("SELECT status FROM tickets WHERE key = 'REAL-1'", [], |r| {
            r.get::<_, String>(0)
        })
        .expect("status")
    };
    // Control: the card is Running, and the journal holds no run of it.
    assert_eq!(status(), "running");
    let r = recover(&store).await.expect("recover");
    assert_eq!(r.lost, ["REAL-1"]);
    assert_eq!(r.running, 0);
    assert_eq!(status(), "blocked");
}

#[tokio::test]
async fn a_release_whose_last_event_carries_no_snapshot_has_no_drift() {
    let (dir, store) = seeded().await;
    let id = one_id(&dir, "SELECT id FROM releases LIMIT 1");
    append_bare_events(&dir, "release", &id, &["note_added"]);
    let r = recover(&store).await.expect("recover");
    assert_eq!(r.drift, Vec::<String>::new());
}
