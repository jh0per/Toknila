//! A card moves to another epic of its project: where it lands, the journal line, and every
//! refusal (nothing changes when one answers).

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};

use super::Api;

/// Project DEMO with the manual epics `src` and `dst` (two cards in a line each: `src` holds
/// DEMO-1 -> DEMO-2, `dst` DEMO-3 -> DEMO-4), the autostart epic `auto`, and project OTHER with
/// its epic `away`. DEMO-5 stands alone in `src`.
async fn setup() -> Api {
    let api = Api::new();
    api.post("/v1/projects", json!({ "key": "DEMO", "name": "Demo" }))
        .await;
    api.post("/v1/projects", json!({ "key": "OTHER", "name": "Other" }))
        .await;
    for (project, name, autostart) in [
        ("DEMO", "src", false),
        ("DEMO", "dst", false),
        ("DEMO", "auto", true),
        ("OTHER", "away", false),
    ] {
        api.post(
            &format!("/v1/projects/{project}/chains"),
            json!({ "name": name, "autostart": autostart }),
        )
        .await;
    }
    for (chain, title) in [("src", "a"), ("src", "b"), ("dst", "c"), ("dst", "d")] {
        api.post(
            &format!("/v1/chains/{chain}/tickets"),
            json!({ "title": title, "at_end": true }),
        )
        .await;
    }
    api.post("/v1/chains/src/tickets", json!({ "title": "alone" }))
        .await;
    api
}

/// The keys of a chain's cards in dependency order.
async fn order(api: &Api, chain: &str) -> Vec<String> {
    let detail = api.get(&format!("/v1/chains/{chain}")).await;
    let tickets = detail["tickets"].as_array().expect("tickets");
    detail["order"]
        .as_array()
        .expect("order")
        .iter()
        .map(|n| {
            let t = tickets.iter().find(|t| t["id"] == n["id"]).expect("card");
            t["key"].as_str().expect("key").to_owned()
        })
        .collect()
}

async fn moved(api: &Api, key: &str, body: Value) -> (StatusCode, Value) {
    api.call(
        Method::POST,
        &format!("/v1/tickets/{key}/move-to-chain"),
        Some(body),
    )
    .await
}

async fn seq(api: &Api) -> i64 {
    let all = api.get("/v1/events?since=0").await;
    all.as_array()
        .expect("events")
        .iter()
        .filter_map(|e| e["seq"].as_i64())
        .max()
        .unwrap_or(0)
}

async fn journal(api: &Api, since: i64) -> Vec<Value> {
    api.get(&format!("/v1/events?since={since}"))
        .await
        .as_array()
        .expect("events")
        .clone()
}

#[tokio::test]
async fn a_card_goes_to_the_end_of_the_other_epic_and_the_journal_says_so() {
    let api = setup().await;
    let before = seq(&api).await;
    let (s, v) = moved(&api, "DEMO-5", json!({ "chain": "dst" })).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["ticket"]["key"], "DEMO-5", "the key stays: {v}");
    assert_eq!(order(&api, "dst").await, ["DEMO-3", "DEMO-4", "DEMO-5"]);
    assert_eq!(order(&api, "src").await, ["DEMO-1", "DEMO-2"]);
    let src = api.get("/v1/chains/src").await;
    assert_eq!(src["tickets"].as_array().map(Vec::len), Some(2));
    let line = journal(&api, before)
        .await
        .into_iter()
        .find(|e| e["entity"] == "ticket" && e["kind"] == "moved")
        .expect("a moved line");
    assert_eq!(line["payload"]["from"], "src", "{line}");
    assert_eq!(line["payload"]["to"], "dst", "{line}");
    assert_eq!(line["payload"]["ticket"]["key"], "DEMO-5", "{line}");
}

#[tokio::test]
async fn a_place_puts_the_card_before_or_after_a_card_of_the_other_epic() {
    let api = setup().await;
    let (s, v) = moved(
        &api,
        "DEMO-5",
        json!({ "chain": "dst", "place": { "anchor": "DEMO-3", "place": "before" } }),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(order(&api, "dst").await, ["DEMO-5", "DEMO-3", "DEMO-4"]);
    api.post("/v1/chains/src/tickets", json!({ "title": "e" }))
        .await;
    let (s, v) = moved(
        &api,
        "DEMO-6",
        json!({ "chain": "dst", "place": { "anchor": "DEMO-3", "place": "after" } }),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(
        order(&api, "dst").await,
        ["DEMO-5", "DEMO-3", "DEMO-6", "DEMO-4"]
    );
}

#[tokio::test]
async fn a_cancelled_card_moves_loose_and_a_place_is_refused_for_it() {
    let api = setup().await;
    api.post("/v1/tickets/DEMO-5/actions/cancel", json!({ "text": "no" }))
        .await;
    let (s, v) = moved(
        &api,
        "DEMO-5",
        json!({ "chain": "dst", "place": { "anchor": "DEMO-3", "place": "after" } }),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{v}");
    let (s, v) = moved(&api, "DEMO-5", json!({ "chain": "dst" })).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["ticket"]["status"], "cancelled");
    let dst = api.get("/v1/chains/dst").await;
    assert_eq!(dst["tickets"].as_array().map(Vec::len), Some(3));
    assert_eq!(dst["edges"].as_array().map(Vec::len), Some(1));
}

#[tokio::test]
async fn every_refusal_names_its_reason_and_changes_nothing() {
    let api = setup().await;
    api.post("/v1/tickets/DEMO-5/actions/start", json!({}))
        .await;
    api.post("/v1/chains/src/tickets", json!({ "title": "e" }))
        .await;
    let before = seq(&api).await;
    let cases = [
        // (card, body, what the answer says)
        ("DEMO-5", json!({ "chain": "dst" }), "running"),
        ("DEMO-6", json!({ "chain": "away" }), "another project"),
        ("DEMO-6", json!({ "chain": "auto" }), "autostart"),
        ("DEMO-6", json!({ "chain": "src" }), "already"),
        ("DEMO-1", json!({ "chain": "dst" }), "DEMO-2"),
        ("DEMO-2", json!({ "chain": "dst" }), "DEMO-1"),
        ("DEMO-6", json!({ "chain": "nope" }), "not found"),
    ];
    for (card, body, says) in cases {
        let (s, v) = moved(&api, card, body.clone()).await;
        assert!(
            s.is_client_error() && v.to_string().contains(says),
            "{card} {body}: {s} {v} (wanted {says})"
        );
    }
    assert_eq!(
        order(&api, "dst").await,
        ["DEMO-3", "DEMO-4"],
        "dst is as it was"
    );
    assert_eq!(
        order(&api, "src").await.len(),
        4,
        "src kept its cards: {:?}",
        order(&api, "src").await
    );
    assert!(
        journal(&api, before)
            .await
            .iter()
            .all(|e| e["kind"] != "moved"),
        "a refusal journals no move"
    );
}

#[tokio::test]
async fn the_planner_moves_a_card_to_another_epic_of_its_project_only() {
    let api = setup().await;
    api.post("/v1/chains/away/tickets", json!({ "title": "theirs" }))
        .await;
    let token = api.planner_token("DEMO").await;
    let post = |chain: &'static str, card: &'static str, body: Value| {
        let (api, token) = (&api, token.clone());
        async move {
            api.call_with(
                &token,
                Method::POST,
                &format!("/v1/planner/chains/{chain}/cards/{card}/move"),
                Some(body),
            )
            .await
        }
    };
    let (s, v) = post("src", "DEMO-5", json!({ "to_chain": "dst" })).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(
        v["chain"]["name"], "dst",
        "the answer is the receiving epic"
    );
    assert_eq!(order(&api, "dst").await, ["DEMO-3", "DEMO-4", "DEMO-5"]);

    api.post("/v1/chains/src/tickets", json!({ "title": "e" }))
        .await;
    let (s, v) = post(
        "src",
        "DEMO-6",
        json!({ "to_chain": "dst", "anchor": "DEMO-3", "place": "before" }),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(
        order(&api, "dst").await,
        ["DEMO-6", "DEMO-3", "DEMO-4", "DEMO-5"]
    );

    api.post("/v1/chains/src/tickets", json!({ "title": "f" }))
        .await;
    for (chain, card, body, status, says) in [
        (
            "src",
            "DEMO-7",
            json!({ "to_chain": "away" }),
            403,
            "another project",
        ),
        (
            "src",
            "DEMO-7",
            json!({ "to_chain": "auto" }),
            403,
            "autostart",
        ),
        (
            "src",
            "DEMO-7",
            json!({ "to_chain": "dst", "anchor": "DEMO-3" }),
            400,
            "both",
        ),
        ("src", "DEMO-7", json!({}), 400, "to_chain"),
        (
            "dst",
            "DEMO-7",
            json!({ "to_chain": "src" }),
            400,
            "not in epic",
        ),
    ] {
        let (s, v) = post(chain, card, body.clone()).await;
        assert_eq!(s.as_u16(), status, "{body}: {v}");
        assert!(v.to_string().contains(says), "{body}: {v}");
    }
    assert_eq!(order(&api, "dst").await.len(), 4, "refusals moved nothing");
}

#[tokio::test]
async fn a_card_with_a_wait_moves_once_the_wait_is_removed() {
    let api = setup().await;
    let (s, _) = moved(&api, "DEMO-2", json!({ "chain": "dst" })).await;
    assert_eq!(s, StatusCode::CONFLICT);
    let (s, v) = api
        .call(
            Method::DELETE,
            "/v1/chains/src/edges?from=DEMO-1&to=DEMO-2",
            None,
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    let (s, v) = moved(&api, "DEMO-2", json!({ "chain": "dst" })).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(order(&api, "dst").await, ["DEMO-3", "DEMO-4", "DEMO-2"]);
}
