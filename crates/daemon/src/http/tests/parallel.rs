//! The parallel agent limit through the full router and the real store, with simulated agents.

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};

use super::Api;

async fn card(api: &Api, key: &str) -> Value {
    api.get(&format!("/v1/tickets/{key}")).await
}

/// `(limit, used, queued keys in line order)` of the only project.
async fn slots(api: &Api) -> (u64, u64, Vec<String>) {
    let agents = api.get("/v1/agents").await;
    let a = &agents[0];
    let mut queued = Vec::new();
    for q in a["queued"].as_array().expect("queued") {
        let id = q["ticket"].as_str().expect("id");
        queued.push(
            card(api, id).await["ticket"]["key"]
                .as_str()
                .expect("key")
                .to_owned(),
        );
    }
    (
        a["limit"].as_u64().expect("limit"),
        a["used"].as_u64().expect("used"),
        queued,
    )
}

async fn manual_chain(api: &Api, cards: usize) {
    api.post("/v1/projects", json!({ "key": "DEMO", "name": "Demo" }))
        .await;
    api.post(
        "/v1/projects/DEMO/chains",
        json!({ "name": "manual", "autostart": false }),
    )
    .await;
    for i in 1..=cards {
        api.post(
            "/v1/chains/manual/tickets",
            json!({ "title": format!("card {i}") }),
        )
        .await;
    }
}

async fn start(api: &Api, key: &str) -> Value {
    api.post(&format!("/v1/tickets/{key}/actions/start"), json!({}))
        .await
}

#[tokio::test]
async fn three_started_cards_run_two_and_queue_one_until_a_slot_frees() {
    let api = Api::new();
    manual_chain(&api, 3).await;
    for key in ["DEMO-1", "DEMO-2", "DEMO-3"] {
        start(&api, key).await;
    }
    assert_eq!(api.status("DEMO-1").await, "running");
    assert_eq!(api.status("DEMO-2").await, "running");
    assert_eq!(api.status("DEMO-3").await, "ready");
    assert_eq!(slots(&api).await, (2, 2, vec!["DEMO-3".to_owned()]));

    // Visible on the card and in the list.
    let third = card(&api, "DEMO-3").await;
    assert_eq!(third["queue_position"], 1);
    assert_eq!(third["ticket"]["queued"], true);
    assert_eq!(card(&api, "DEMO-1").await["queue_position"], Value::Null);
    let items = api.get("/v1/tickets").await;
    let positions: Vec<(String, Value)> = items
        .as_array()
        .expect("items")
        .iter()
        .map(|i| {
            let key = i["ticket"]["key"].as_str().expect("key").to_owned();
            (key, i["queue_position"].clone())
        })
        .collect();
    assert_eq!(
        positions,
        [
            ("DEMO-1".to_owned(), Value::Null),
            ("DEMO-2".to_owned(), Value::Null),
            ("DEMO-3".to_owned(), json!(1)),
        ]
    );

    // The first agent hands in: Review frees its slot and the queued card starts.
    api.post(
        "/v1/tickets/DEMO-1/events",
        json!({ "event": "finished", "summary": toknila_core::handin::from_plain("done") }),
    )
    .await;
    assert_eq!(api.status("DEMO-1").await, "review");
    assert_eq!(api.status("DEMO-3").await, "running");
    assert_eq!(slots(&api).await, (2, 2, vec![]));
    assert_eq!(card(&api, "DEMO-3").await["ticket"]["queued"], false);
}

#[tokio::test]
async fn changing_the_limit_starts_the_queue_up_and_never_stops_agents_down() {
    let api = Api::new();
    manual_chain(&api, 4).await;
    for key in ["DEMO-1", "DEMO-2", "DEMO-3", "DEMO-4"] {
        start(&api, key).await;
    }
    assert_eq!(
        slots(&api).await,
        (2, 2, vec!["DEMO-3".to_owned(), "DEMO-4".to_owned()])
    );

    let up = api
        .call(
            Method::PATCH,
            "/v1/projects/DEMO",
            Some(json!({ "max_parallel": 3 })),
        )
        .await;
    assert_eq!((up.0, &up.1["max_parallel"]), (StatusCode::OK, &json!(3)));
    assert_eq!(api.status("DEMO-3").await, "running");
    assert_eq!(slots(&api).await, (3, 3, vec!["DEMO-4".to_owned()]));

    let down = api
        .call(
            Method::PATCH,
            "/v1/projects/DEMO",
            Some(json!({ "max_parallel": 1 })),
        )
        .await;
    assert_eq!(down.0, StatusCode::OK);
    for key in ["DEMO-1", "DEMO-2", "DEMO-3"] {
        assert_eq!(api.status(key).await, "running", "{key} was stopped");
    }
    assert_eq!(slots(&api).await, (1, 3, vec!["DEMO-4".to_owned()]));

    // The limit is stored with the project and survives a read.
    assert_eq!(api.get("/v1/projects/DEMO").await["max_parallel"], 1);
    let kinds: Vec<String> = api
        .get("/v1/events")
        .await
        .as_array()
        .expect("events")
        .iter()
        .filter(|e| e["entity"] == "project")
        .map(|e| e["kind"].as_str().expect("kind").to_owned())
        .collect();
    assert_eq!(kinds, ["created", "updated", "updated"]);
}

#[tokio::test]
async fn a_new_project_has_two_slots_and_a_bad_limit_is_refused() {
    let api = Api::new();
    manual_chain(&api, 1).await;
    assert_eq!(api.get("/v1/projects/DEMO").await["max_parallel"], 2);
    for bad in [0, 65] {
        let (s, body) = api
            .call(
                Method::PATCH,
                "/v1/projects/DEMO",
                Some(json!({ "max_parallel": bad })),
            )
            .await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{body}");
        assert_eq!(
            body["error"],
            format!("parallel agents must be between 1 and 64, not {bad}")
        );
    }
    assert_eq!(api.get("/v1/projects/DEMO").await["max_parallel"], 2);
}

#[tokio::test]
async fn cards_of_an_autostart_chain_queue_in_graph_order_across_chains() {
    let api = Api::new();
    api.demo().await;
    api.post(
        "/v1/projects/DEMO/chains",
        json!({ "name": "later", "autostart": true }),
    )
    .await;
    for title in ["a", "b", "c"] {
        api.post("/v1/chains/demo/tickets", json!({ "title": title }))
            .await;
    }
    api.post("/v1/chains/later/tickets", json!({ "title": "d" }))
        .await;
    // Two of the first chain run; its third card and the other chain's card wait, in that order.
    assert_eq!(api.status("DEMO-3").await, "ready");
    assert_eq!(
        slots(&api).await,
        (2, 2, vec!["DEMO-3".to_owned(), "DEMO-4".to_owned()])
    );
}
