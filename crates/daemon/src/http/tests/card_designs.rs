//! A card's designs through the full router: the planner sets and changes them by file name, the
//! card page by id, a design the project lacks is refused, and a chain file carries them by name.

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};

use super::Api;
use crate::designs::tests::PNG;

async fn upload(api: &Api, name: &str) -> String {
    let (s, _, bytes) = super::designs::raw(
        api,
        Method::POST,
        &format!("/v1/projects/DEMO/designs?name={name}"),
        PNG,
    )
    .await;
    assert_eq!(s, StatusCode::CREATED);
    let v: Value = serde_json::from_slice(&bytes).expect("json");
    v["id"].as_str().expect("id").to_owned()
}

fn refs(card: &Value) -> Vec<(String, Option<u64>)> {
    card["ticket"]["designs"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .map(|d| {
            (
                d["design"].as_str().expect("id").to_owned(),
                d["region"]["w"].as_u64(),
            )
        })
        .collect()
}

#[tokio::test]
async fn the_planner_sets_and_changes_designs_by_name_and_a_missing_one_is_refused() {
    let api = Api::new();
    api.two_projects().await;
    let login = upload(&api, "login.png").await;
    let home = upload(&api, "home.png").await;
    let token = api.planner_token("DEMO").await;

    let (s, card) = api
        .call_with(
            &token,
            Method::POST,
            "/v1/planner/chains/plan/cards",
            Some(json!({ "title": "Login screen", "designs": [
                { "design": "login.png", "region": { "x": 20, "y": 10, "w": 40, "h": 30 } },
                { "design": home },
            ]})),
        )
        .await;
    assert_eq!(s, StatusCode::CREATED, "{card}");
    // The bytes of the answer: ids, and the region as percents.
    assert_eq!(
        card["ticket"]["designs"],
        json!([
            { "design": login, "region": { "x": 20, "y": 10, "w": 40, "h": 30 } },
            { "design": home },
        ])
    );
    let key = card["ticket"]["key"].as_str().expect("key").to_owned();

    // The card page reads them back, and a restart keeps them.
    let api = api.restart();
    assert_eq!(
        refs(&api.get(&format!("/v1/tickets/{key}")).await),
        [(login.clone(), Some(40)), (home.clone(), None)]
    );

    let token = api.planner_token("DEMO").await;
    // edit_card replaces the list; leaving it out keeps it; [] clears it.
    let uri = format!("/v1/planner/cards/{key}");
    let (s, v) = api
        .call_with(
            &token,
            Method::PATCH,
            &uri,
            Some(json!({ "designs": [{ "design": "home.png" }] })),
        )
        .await;
    assert_eq!(
        (s, refs(&v)),
        (StatusCode::OK, vec![(home.clone(), None)]),
        "{v}"
    );
    let (_, v) = api
        .call_with(
            &token,
            Method::PATCH,
            &uri,
            Some(json!({ "title": "Login" })),
        )
        .await;
    assert_eq!(refs(&v), [(home.clone(), None)]);
    let (_, v) = api
        .call_with(&token, Method::PATCH, &uri, Some(json!({ "designs": [] })))
        .await;
    assert!(refs(&v).is_empty(), "{v}");

    // Controls: an unknown name names the library; a region outside the image is refused too.
    let (s, e) = api
        .call_with(
            &token,
            Method::PATCH,
            &uri,
            Some(json!({ "designs": [{ "design": "nope.png" }] })),
        )
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{e}");
    let text = e["error"].as_str().expect("message");
    assert!(
        text.contains("nope.png") && text.contains("login.png"),
        "{e}"
    );
    let (s, _) = api
        .call_with(
            &token,
            Method::PATCH,
            &uri,
            Some(json!({ "designs": [{ "design": "home.png", "region": { "x": 80, "y": 0, "w": 30, "h": 5 } }] })),
        )
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let (_, v) = api
        .call_with(&token, Method::PATCH, &uri, Some(json!({})))
        .await;
    assert!(refs(&v).is_empty(), "nothing was changed by the refusals");
}

#[tokio::test]
async fn the_card_page_sets_designs_by_id_from_the_library() {
    let api = Api::new();
    api.demo().await;
    let login = upload(&api, "login.png").await;
    let card = api
        .post("/v1/chains/demo/tickets", json!({ "title": "A" }))
        .await;
    let key = card["ticket"]["key"].as_str().expect("key").to_owned();
    let (s, v) = api
        .call(
            Method::PATCH,
            &format!("/v1/tickets/{key}"),
            Some(json!({ "designs": [{ "design": login, "region": { "x": 0, "y": 0, "w": 50, "h": 50 } }] })),
        )
        .await;
    assert_eq!(
        (s, refs(&v)),
        (StatusCode::OK, vec![(login.clone(), Some(50))]),
        "{v}"
    );
    // A design the project does not have (here: one that was never kept) is refused.
    let ghost = toknila_core::DesignId::new().to_string();
    let (s, e) = api
        .call(
            Method::PATCH,
            &format!("/v1/tickets/{key}"),
            Some(json!({ "designs": [{ "design": ghost }] })),
        )
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{e}");
    assert_eq!(refs(&api.get(&format!("/v1/tickets/{key}")).await).len(), 1);
}

#[tokio::test]
async fn a_chain_file_carries_designs_by_name_and_an_unknown_name_keeps_the_card_with_a_warning() {
    let api = Api::new();
    api.demo().await;
    let login = upload(&api, "login.png").await;
    let card = api
        .post(
            "/v1/chains/demo/tickets",
            json!({ "title": "A", "designs": [
                { "design": login, "region": { "x": 20, "y": 10, "w": 40, "h": 30 } }] }),
        )
        .await;
    assert_eq!(refs(&card).len(), 1);
    let toml = api.get("/v1/chains/demo/export").await["toml"]
        .as_str()
        .expect("toml")
        .to_owned();
    assert!(
        toml.contains(
            r#"designs = [{ design = "login.png", region = { x = 20, y = 10, w = 40, h = 30 } }]"#
        ),
        "{toml}"
    );
    assert!(!toml.contains(&login), "names, never ids: {toml}");

    // Into a project that has the same design name: the reference is its own id.
    api.post("/v1/projects", json!({ "key": "SAME" })).await;
    let (s, _, bytes) = super::designs::raw(
        &api,
        Method::POST,
        "/v1/projects/SAME/designs?name=login.png",
        PNG,
    )
    .await;
    assert_eq!(s, StatusCode::CREATED);
    let own: Value = serde_json::from_slice(&bytes).expect("json");
    let (s, v) = api
        .call(
            Method::POST,
            "/v1/projects/SAME/import",
            Some(json!({ "toml": toml })),
        )
        .await;
    assert_eq!(s, StatusCode::CREATED, "{v}");
    assert_eq!(v["tickets"][0]["designs"][0]["design"], own["id"], "{v}");
    assert!(v.get("warnings").is_none(), "{v}");

    // Into a project without it: the card is made, the reference is dropped, a warning says so.
    api.post("/v1/projects", json!({ "key": "NONE" })).await;
    let (s, v) = api
        .call(
            Method::POST,
            "/v1/projects/NONE/import",
            Some(json!({ "toml": toml })),
        )
        .await;
    assert_eq!(s, StatusCode::CREATED, "{v}");
    assert_eq!(v["tickets"].as_array().map(Vec::len), Some(1));
    assert!(v["tickets"][0].get("designs").is_none(), "{v}");
    let warning = v["warnings"][0].as_str().expect("warning");
    assert!(
        warning.contains("NONE-1") && warning.contains("login.png"),
        "{warning}"
    );
}
