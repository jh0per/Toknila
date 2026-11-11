//! The assistant's pins through the full router: the planner pins a question on a design of its
//! project, the person answers it, the answer reaches the card's prompt and the plan, and the
//! open pin is in the inbox until then.

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};
use toknila_core::ProjectId;

use super::Api;
use crate::designs::Designs;
use crate::designs::tests::PNG;
use crate::server::now;

async fn setup() -> (Api, String) {
    let api = Api::new();
    api.two_projects().await;
    api.post(
        "/v1/chains/plan/tickets",
        json!({ "title": "Checkout page", "prompt": "Build the page." }),
    )
    .await;
    let id = api.get("/v1/projects/DEMO").await["id"]
        .as_str()
        .and_then(|s| s.parse::<ProjectId>().ok())
        .expect("project id");
    Designs::new(api._dir.path(), id)
        .add_file("home.png", PNG, now())
        .expect("png");
    let token = api.planner_token("DEMO").await;
    (api, token)
}

fn question(card: Value) -> Value {
    json!({ "design": "HOME.png", "region": { "x": 10, "y": 20, "w": 30, "h": 40 },
            "text": "Show the tax separately?", "card": card })
}

#[tokio::test]
async fn the_planner_pins_a_question_and_the_person_answers_it_into_the_prompt() {
    let (api, token) = setup().await;
    let (s, pin) = api
        .call_with(
            &token,
            Method::POST,
            "/v1/planner/pins",
            Some(question(json!("DEMO-1"))),
        )
        .await;
    assert_eq!(s, StatusCode::CREATED, "{pin}");
    assert_eq!(pin["number"], 1);
    assert_eq!(pin["design_name"], "home.png");
    assert_eq!(pin["card"], "DEMO-1");
    assert_eq!(pin["answer"], Value::Null);

    // The window lists it, open; the inbox asks the person; the plan shows it to the planner.
    let listed = api.get("/v1/projects/DEMO/design-pins").await;
    assert_eq!(listed[0]["id"], pin["id"]);
    let inbox = api.get("/v1/projects/DEMO/inbox").await;
    let item = inbox
        .as_array()
        .and_then(|a| a.iter().find(|i| i["kind"] == "design_question"))
        .unwrap_or_else(|| panic!("no design_question in {inbox}"));
    assert_eq!(item["ticket"], "DEMO-1");
    assert_eq!(item["pin"], pin["id"]);
    assert_eq!(item["text"], "home.png: Show the tax separately?");
    let (_, plan) = api
        .call_with(&token, Method::GET, "/v1/planner/plan", None)
        .await;
    assert_eq!(plan["pins"][0]["text"], "Show the tax separately?");
    assert_eq!(plan["pins"][0]["answer"], Value::Null);

    // The person answers in place.
    let uri = format!(
        "/v1/projects/DEMO/design-pins/{}/answer",
        pin["id"].as_str().expect("id")
    );
    let done = api
        .post(&uri, json!({ "text": "Yes, on its own line." }))
        .await;
    assert_eq!(done["answer"], "Yes, on its own line.");
    let card = api.get("/v1/tickets/DEMO-1").await;
    assert_eq!(
        card["ticket"]["prompt"],
        "Build the page.\n\n## From the design\n- home.png #1 (x 10%, y 20%, 30% wide, 40% tall): Show the tax separately? - Yes, on its own line."
    );
    let inbox = api.get("/v1/projects/DEMO/inbox").await;
    assert!(
        inbox
            .as_array()
            .expect("inbox")
            .iter()
            .all(|i| i["kind"] != "design_question"),
        "an answered pin leaves the inbox: {inbox}"
    );
    let (_, plan) = api
        .call_with(&token, Method::GET, "/v1/planner/plan", None)
        .await;
    assert_eq!(plan["pins"][0]["answer"], "Yes, on its own line.");
}

#[tokio::test]
async fn bad_pins_and_foreign_answers_are_refused() {
    let (api, token) = setup().await;
    let pin = |body: Value| api.call_with(&token, Method::POST, "/v1/planner/pins", Some(body));
    let mut zero = question(json!("DEMO-1"));
    zero["region"]["w"] = json!(0);
    assert_eq!(pin(zero).await.0, StatusCode::BAD_REQUEST);
    let mut missing = question(json!("DEMO-1"));
    missing["design"] = json!("nothing.png");
    assert_eq!(pin(missing).await.0, StatusCode::NOT_FOUND);
    // A card of another project is out of the planner's reach.
    assert_eq!(
        pin(question(json!("OTHER-1"))).await.0,
        StatusCode::FORBIDDEN
    );
    // A person's token is not the planner's.
    let (s, _) = api
        .call_with(
            super::TOKEN,
            Method::POST,
            "/v1/planner/pins",
            Some(question(json!(null))),
        )
        .await;
    assert_eq!(s, StatusCode::FORBIDDEN);

    let (s, made) = pin(question(json!(null))).await;
    assert_eq!(s, StatusCode::CREATED, "{made}");
    let id = made["id"].as_str().expect("id");
    // The other project cannot answer it, and an empty answer is refused.
    let (s, _) = api
        .call(
            Method::POST,
            &format!("/v1/projects/OTHER/design-pins/{id}/answer"),
            Some(json!({ "text": "x" })),
        )
        .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (s, _) = api
        .call(
            Method::POST,
            &format!("/v1/projects/DEMO/design-pins/{id}/answer"),
            Some(json!({ "text": "  " })),
        )
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
}
