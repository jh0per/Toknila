//! The roadmap through the full router: chains are listed in order, a person moves one, the next
//! version follows the planned ones, and the planner makes and edits epics of its own project.

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};

use super::Api;

async fn demo(names: &[&str]) -> Api {
    let api = Api::new();
    api.post("/v1/projects", json!({ "key": "DEMO", "name": "Demo" }))
        .await;
    for n in names {
        api.post("/v1/projects/DEMO/chains", json!({ "name": n }))
            .await;
    }
    api
}

async fn order(api: &Api) -> Vec<String> {
    api.get("/v1/chains")
        .await
        .as_array()
        .expect("list")
        .iter()
        .map(|c| c["chain"]["name"].as_str().unwrap_or_default().to_owned())
        .collect()
}

async fn mv(api: &Api, chain: &str, body: Value) -> (StatusCode, Value) {
    api.call(
        Method::POST,
        &format!("/v1/chains/{chain}/move"),
        Some(body),
    )
    .await
}

#[tokio::test]
async fn new_chains_land_at_the_end_and_a_move_puts_one_before_or_after_another() {
    let api = demo(&["one", "two", "three", "four"]).await;
    assert_eq!(order(&api).await, ["one", "two", "three", "four"]);

    let (s, c) = mv(&api, "four", json!({ "before": "two" })).await;
    assert_eq!(s, StatusCode::OK, "{c}");
    assert_eq!(c["chain"]["position"], json!(1), "{c}");
    assert_eq!(order(&api).await, ["one", "four", "two", "three"]);

    let (s, c) = mv(&api, "one", json!({ "after": "three" })).await;
    assert_eq!(s, StatusCode::OK, "{c}");
    assert_eq!(order(&api).await, ["four", "two", "three", "one"]);

    // A new chain still goes to the end, after a move.
    api.post("/v1/projects/DEMO/chains", json!({ "name": "five" }))
        .await;
    assert_eq!(order(&api).await, ["four", "two", "three", "one", "five"]);

    // The move is journaled as the chain's update.
    let events = api.get("/v1/events?since=0").await;
    let updated = events
        .as_array()
        .expect("events")
        .iter()
        .filter(|e| e["entity"] == "chain" && e["kind"] == "updated")
        .count();
    assert!(updated >= 2, "{events}");
}

#[tokio::test]
async fn an_imported_plan_lands_at_the_end_of_the_roadmap() {
    let api = demo(&["one", "two"]).await;
    mv(&api, "two", json!({ "before": "one" })).await;
    let plan = "[chain]\nname = \"imported\"\n\n[[ticket]]\nid = \"A\"\n";
    let c = api
        .post("/v1/projects/DEMO/import", json!({ "toml": plan }))
        .await;
    assert_eq!(c["chain"]["position"], json!(2), "{c}");
    assert_eq!(order(&api).await, ["two", "one", "imported"]);
}

#[tokio::test]
async fn a_finished_epic_moves_too_and_a_bad_move_is_refused_and_changes_nothing() {
    let api = demo(&["one", "two"]).await;
    api.post("/v1/chains/one/tickets", json!({ "title": "A" }))
        .await;
    api.post(
        "/v1/tickets/DEMO-1/actions/cancel",
        json!({ "text": "not needed" }),
    )
    .await;
    let (s, c) = mv(&api, "one", json!({ "after": "two" })).await;
    assert_eq!(s, StatusCode::OK, "{c}");
    assert_eq!(order(&api).await, ["two", "one"]);

    for body in [
        json!({}),
        json!({ "before": "two", "after": "one" }),
        json!({ "before": "one" }),
        json!({ "before": "nope" }),
    ] {
        let (s, e) = mv(&api, "one", body.clone()).await;
        assert!(
            s == StatusCode::BAD_REQUEST || s == StatusCode::NOT_FOUND,
            "{body}: {s} {e}"
        );
    }
    assert_eq!(order(&api).await, ["two", "one"]);
}

#[tokio::test]
async fn the_next_version_follows_the_greatest_planned_version() {
    let api = demo(&[]).await;
    let next = || async { api.get("/v1/projects/DEMO/next-version").await["version"].clone() };
    assert_eq!(next().await, json!("0.1.0"));
    api.post(
        "/v1/projects/DEMO/chains",
        json!({ "name": "a", "version": "0.6.0" }),
    )
    .await;
    api.post(
        "/v1/projects/DEMO/chains",
        json!({ "name": "b", "version": "0.4.0" }),
    )
    .await;
    assert_eq!(next().await, json!("0.6.1"));
}

#[tokio::test]
async fn the_planner_makes_an_epic_with_a_version_and_edits_it_in_its_own_project_only() {
    let api = Api::new();
    api.two_projects().await;
    let token = api.planner_token("DEMO").await;
    let (s, c) = api
        .call_with(
            &token,
            Method::POST,
            "/v1/planner/chains",
            Some(json!({ "name": "phase-2", "goal": "g", "version": "v0.2.0" })),
        )
        .await;
    assert_eq!(s, StatusCode::CREATED, "{c}");
    assert_eq!(c["chain"]["version"], json!("0.2.0"), "{c}");
    assert_eq!(c["chain"]["position"], json!(1), "at the end: {c}");

    let (s, c) = api
        .call_with(
            &token,
            Method::PATCH,
            "/v1/planner/chains/phase-2",
            Some(json!({ "name": "phase-two", "goal": "new goal", "version": "0.3.0" })),
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{c}");
    assert_eq!(
        (
            &c["chain"]["name"],
            &c["chain"]["goal"],
            &c["chain"]["version"]
        ),
        (&json!("phase-two"), &json!("new goal"), &json!("0.3.0"))
    );

    let (s, e) = api
        .call_with(
            &token,
            Method::PATCH,
            "/v1/planner/chains/theirs",
            Some(json!({ "goal": "mine now" })),
        )
        .await;
    assert_eq!(s, StatusCode::FORBIDDEN, "{e}");
    let theirs = api.get("/v1/chains/theirs").await;
    assert_eq!(theirs["chain"]["goal"], json!(""), "{theirs}");
}
