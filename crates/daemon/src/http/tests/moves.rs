//! Moving a dependency or a design reference is one request: it lands whole, or nothing changes.

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};

use super::Api;
use crate::designs::tests::PNG;

async fn three_paused(api: &Api) {
    api.demo().await;
    for t in ["A", "B", "C"] {
        api.post("/v1/chains/demo/tickets", json!({ "title": t }))
            .await;
    }
    for k in ["DEMO-1", "DEMO-2", "DEMO-3"] {
        api.post(&format!("/v1/tickets/{k}/actions/pause"), json!({}))
            .await;
    }
}

fn pairs(chain: &Value) -> Vec<String> {
    let tickets = chain["tickets"].as_array().expect("tickets");
    let key = |id: &Value| {
        tickets.iter().find(|t| t["id"] == *id).map_or_else(
            || id.to_string(),
            |t| t["key"].as_str().expect("key").to_owned(),
        )
    };
    chain["edges"]
        .as_array()
        .expect("edges")
        .iter()
        .map(|e| format!("{}>{}", key(&e["from"]["id"]), key(&e["to"]["id"])))
        .collect()
}

async fn edges(api: &Api) -> Vec<String> {
    pairs(&api.get("/v1/chains/demo").await)
}

async fn kinds(api: &Api, since: i64) -> Vec<String> {
    api.get(&format!("/v1/events?since={since}"))
        .await
        .as_array()
        .expect("array")
        .iter()
        .map(|e| {
            format!(
                "{}.{}",
                e["entity"].as_str().unwrap_or(""),
                e["kind"].as_str().unwrap_or("")
            )
        })
        .collect()
}

async fn last_seq(api: &Api) -> i64 {
    api.get("/v1/events?since=0")
        .await
        .as_array()
        .expect("array")
        .iter()
        .filter_map(|e| e["seq"].as_i64())
        .max()
        .unwrap_or(0)
}

fn edge_move(of: &str, ot: &str, nf: &str, nt: &str) -> Value {
    json!({ "old": { "from": of, "to": ot }, "new": { "from": nf, "to": nt } })
}

#[tokio::test]
async fn an_edge_move_lands_whole_as_one_event() {
    let api = Api::new();
    three_paused(&api).await;
    api.post(
        "/v1/chains/demo/edges",
        json!({ "from": "DEMO-1", "to": "DEMO-2" }),
    )
    .await;
    let seq = last_seq(&api).await;
    let c = api
        .post(
            "/v1/chains/demo/edges/move",
            edge_move("DEMO-1", "DEMO-2", "DEMO-1", "DEMO-3"),
        )
        .await;
    assert_eq!(pairs(&c), ["DEMO-1>DEMO-3"]);
    assert_eq!(edges(&api).await, ["DEMO-1>DEMO-3"]);
    assert_eq!(kinds(&api, seq).await, ["chain.edge_moved"]);
    let ev = api.get(&format!("/v1/events?since={seq}")).await;
    // The bytes of the event: the old edge ended at DEMO-2, the new one at DEMO-3.
    let id = |k: &str| {
        let t = c["tickets"].as_array().expect("tickets");
        t.iter().find(|t| t["key"] == k).expect("ticket")["id"].clone()
    };
    assert_eq!(ev[0]["payload"]["was"]["to"]["id"], id("DEMO-2"), "{ev}");
    assert_eq!(ev[0]["payload"]["edge"]["to"]["id"], id("DEMO-3"), "{ev}");
}

#[tokio::test]
async fn a_refused_cycle_or_a_missing_end_leaves_the_edges_and_the_journal_alone() {
    let api = Api::new();
    three_paused(&api).await;
    for (f, t) in [("DEMO-1", "DEMO-2"), ("DEMO-2", "DEMO-3")] {
        api.post("/v1/chains/demo/edges", json!({ "from": f, "to": t }))
            .await;
    }
    let before = edges(&api).await;
    assert_eq!(before, ["DEMO-1>DEMO-2", "DEMO-2>DEMO-3"]);
    let seq = last_seq(&api).await;

    // DEMO-3 -> DEMO-2 would close a cycle with the edge that stays.
    let (s, e) = api
        .call(
            Method::POST,
            "/v1/chains/demo/edges/move",
            Some(edge_move("DEMO-1", "DEMO-2", "DEMO-3", "DEMO-2")),
        )
        .await;
    assert_eq!(s, StatusCode::CONFLICT, "{e}");
    assert!(e["error"].as_str().expect("error").contains("cycle"), "{e}");

    // A new end the chain does not have.
    let (s, _) = api
        .call(
            Method::POST,
            "/v1/chains/demo/edges/move",
            Some(edge_move("DEMO-1", "DEMO-2", "DEMO-1", "DEMO-9")),
        )
        .await;
    assert_eq!(s, StatusCode::NOT_FOUND);

    // An old edge that is not there.
    let (s, _) = api
        .call(
            Method::POST,
            "/v1/chains/demo/edges/move",
            Some(edge_move("DEMO-3", "DEMO-1", "DEMO-1", "DEMO-3")),
        )
        .await;
    assert_eq!(s, StatusCode::NOT_FOUND);

    // An unknown field is refused too.
    let mut bad = edge_move("DEMO-1", "DEMO-2", "DEMO-1", "DEMO-3");
    bad["extra"] = json!(1);
    let (s, _) = api
        .call(Method::POST, "/v1/chains/demo/edges/move", Some(bad))
        .await;
    assert!(s.is_client_error(), "{s}");

    assert_eq!(edges(&api).await, before);
    assert_eq!(kinds(&api, seq).await, Vec::<String>::new());
}

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

async fn designs_of(api: &Api, key: &str) -> Value {
    api.get(&format!("/v1/tickets/{key}")).await["ticket"]["designs"].clone()
}

#[tokio::test]
async fn a_design_move_keeps_the_region_and_is_one_event_and_a_missing_target_changes_nothing() {
    let api = Api::new();
    api.demo().await;
    for t in ["A", "B"] {
        api.post("/v1/chains/demo/tickets", json!({ "title": t }))
            .await;
    }
    let login = upload(&api, "login.png").await;
    let home = upload(&api, "home.png").await;
    let region = json!({ "x": 20, "y": 10, "w": 40, "h": 30 });
    let list = json!([{ "design": login, "region": region }, { "design": home }]);
    let (s, v) = api
        .call(
            Method::PATCH,
            "/v1/tickets/DEMO-1",
            Some(json!({ "designs": list })),
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    let seq = last_seq(&api).await;

    // A target the chain lacks, and an index past the list: nothing changes.
    let (s, _) = api
        .call(
            Method::POST,
            "/v1/tickets/DEMO-1/designs/0/move",
            Some(json!({ "target": "DEMO-9" })),
        )
        .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (s, _) = api
        .call(
            Method::POST,
            "/v1/tickets/DEMO-1/designs/5/move",
            Some(json!({ "target": "DEMO-2" })),
        )
        .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    assert_eq!(designs_of(&api, "DEMO-1").await, list);
    // An empty list is left out of the card's JSON.
    assert_eq!(designs_of(&api, "DEMO-2").await, Value::Null);
    assert_eq!(kinds(&api, seq).await, Vec::<String>::new());

    let (s, v) = api
        .call(
            Method::POST,
            "/v1/tickets/DEMO-1/designs/0/move",
            Some(json!({ "target": "DEMO-2" })),
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(
        designs_of(&api, "DEMO-1").await,
        json!([{ "design": home }])
    );
    assert_eq!(
        designs_of(&api, "DEMO-2").await,
        json!([{ "design": login, "region": region }])
    );
    assert_eq!(kinds(&api, seq).await, ["ticket.design_moved"]);
}
