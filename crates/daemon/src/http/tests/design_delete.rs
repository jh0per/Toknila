//! Removing a design of the library through the full router: refused with the users named, forced
//! with the references and open pins gone and a comment on each card; and the first-page
//! thumbnail of a pdf (a fake renderer stands for the machine's).

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};

use super::Api;
use crate::designs::tests::PNG;

async fn upload(api: &Api, name: &str, bytes: &[u8]) -> Value {
    let (s, _, body) = super::designs::raw(
        api,
        Method::POST,
        &format!("/v1/projects/DEMO/designs?name={name}"),
        bytes,
    )
    .await;
    assert_eq!(s, StatusCode::CREATED);
    serde_json::from_slice(&body).expect("json")
}

async fn raw_get(api: &Api, uri: &str) -> (StatusCode, String, Vec<u8>) {
    super::designs::raw(api, Method::GET, uri, b"").await
}

/// A project with two cards that show `home.png` (DEMO-1, DEMO-2) and two open pins on it, one
/// of DEMO-1 and one of no card; answers the id of the design and the planner's token.
async fn setup(api: &Api) -> (String, String) {
    api.two_projects().await;
    let id = upload(api, "home.png", PNG).await["id"]
        .as_str()
        .expect("id")
        .to_owned();
    let token = api.planner_token("DEMO").await;
    for title in ["Checkout", "Login"] {
        let (s, v) = api
            .call_with(
                &token,
                Method::POST,
                "/v1/planner/chains/plan/cards",
                Some(json!({ "title": title, "designs": [{ "design": "home.png" }] })),
            )
            .await;
        assert_eq!(s, StatusCode::CREATED, "{v}");
    }
    for card in [json!("DEMO-1"), Value::Null] {
        let (s, v) = api
            .call_with(
                &token,
                Method::POST,
                "/v1/planner/pins",
                Some(
                    json!({ "design": "home.png", "text": "Which logo?", "card": card,
                    "region": { "x": 1, "y": 2, "w": 3, "h": 4 } }),
                ),
            )
            .await;
        assert_eq!(s, StatusCode::CREATED, "{v}");
    }
    (id, token)
}

#[tokio::test]
async fn a_design_in_use_is_refused_with_its_users_named() {
    let api = Api::new();
    let (id, _) = setup(&api).await;
    let uri = format!("/v1/projects/DEMO/designs/{id}");

    let (s, body) = api.call(Method::DELETE, &uri, None).await;
    assert_eq!(s, StatusCode::CONFLICT, "{body}");
    assert_eq!(
        body,
        json!({
            "error": "home.png is used by DEMO-1, DEMO-2 and 2 pins.",
            "code": "conflict",
            "used_by": { "cards": ["DEMO-1", "DEMO-2"], "pins": 2 },
        })
    );
    // Nothing moved: the design, the references and the pins are all there.
    let list = api.get("/v1/projects/DEMO/designs").await;
    assert_eq!(list["designs"].as_array().map(Vec::len), Some(1));
    let card = api.get("/v1/tickets/DEMO-1").await;
    assert_eq!(card["ticket"]["designs"][0]["design"], id.as_str());
    let pins = api.get("/v1/projects/DEMO/design-pins").await;
    assert_eq!(pins.as_array().map(Vec::len), Some(2));
}

#[tokio::test]
async fn forced_it_removes_references_and_open_pins_and_comments_on_the_cards() {
    let api = Api::new();
    let (id, _) = setup(&api).await;
    let uri = format!("/v1/projects/DEMO/designs/{id}?force=true");

    let (s, body) = api.call(Method::DELETE, &uri, None).await;
    assert_eq!(s, StatusCode::NO_CONTENT, "{body}");

    let list = api.get("/v1/projects/DEMO/designs").await;
    assert_eq!(list["designs"], json!([]));
    let (s, _, _) = raw_get(&api, &format!("/v1/projects/DEMO/designs/{id}/file")).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    assert_eq!(api.get("/v1/projects/DEMO/design-pins").await, json!([]));
    assert_eq!(api.get("/v1/projects/DEMO/inbox").await, json!([]));
    for key in ["DEMO-1", "DEMO-2"] {
        let card = api.get(&format!("/v1/tickets/{key}")).await;
        // An empty list is left out of the card's JSON.
        assert_eq!(card["ticket"]["designs"], Value::Null, "{key}");
        let said: Vec<_> = card["comments"]
            .as_array()
            .expect("comments")
            .iter()
            .filter(|c| c["author"] == "system")
            .collect();
        assert_eq!(said.len(), 1, "{key}: {card}");
        assert_eq!(
            said[0]["body"]["gist"],
            "Design home.png was removed from the library"
        );
    }
}

#[tokio::test]
async fn a_free_design_is_removed_without_force() {
    let api = Api::new();
    api.two_projects().await;
    let id = upload(&api, "home.png", PNG).await["id"]
        .as_str()
        .expect("id")
        .to_owned();
    let uri = format!("/v1/projects/DEMO/designs/{id}");
    let (s, _) = api.call(Method::DELETE, &uri, None).await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    let (s, _) = api.call(Method::DELETE, &uri, None).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_pdf_gets_the_first_page_as_its_thumbnail() {
    let api = Api::new();
    api.two_projects().await;
    *api.pdf_page.lock().expect("page") = Some(PNG.to_vec());
    let made = upload(&api, "flow.pdf", b"%PDF-1.4 x").await;
    assert_eq!(
        (made["kind"].as_str(), made["thumb"].as_bool()),
        (Some("pdf"), Some(true))
    );
    let id = made["id"].as_str().expect("id").to_owned();

    let (s, ctype, bytes) = raw_get(&api, &format!("/v1/projects/DEMO/designs/{id}/thumb")).await;
    assert_eq!(
        (s, ctype.as_str(), bytes.as_slice()),
        (StatusCode::OK, "image/png", PNG)
    );
    // The file itself is still the pdf, for the viewer.
    let (s, ctype, bytes) = raw_get(&api, &format!("/v1/projects/DEMO/designs/{id}/file")).await;
    assert_eq!(
        (s, ctype.as_str(), bytes.as_slice()),
        (StatusCode::OK, "application/pdf", &b"%PDF-1.4 x"[..])
    );
    // The list says so after a restart too.
    let api = api.restart();
    assert_eq!(
        api.get("/v1/projects/DEMO/designs").await["designs"][0]["thumb"],
        true
    );
    // Removing the design takes the thumbnail with it.
    let uri = format!("/v1/projects/DEMO/designs/{id}");
    let (s, _) = api.call(Method::DELETE, &uri, None).await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    let (s, _, _) = raw_get(&api, &format!("{uri}/thumb")).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn without_a_renderer_a_pdf_is_kept_with_no_thumbnail() {
    let api = Api::new();
    api.two_projects().await;
    let made = upload(&api, "flow.pdf", b"%PDF-1.4 x").await;
    assert_eq!(
        (made["kind"].as_str(), made["thumb"].as_bool()),
        (Some("pdf"), Some(false))
    );
    let id = made["id"].as_str().expect("id");
    let (s, _, _) = raw_get(&api, &format!("/v1/projects/DEMO/designs/{id}/thumb")).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (s, _, bytes) = raw_get(&api, &format!("/v1/projects/DEMO/designs/{id}/file")).await;
    assert_eq!((s, bytes.as_slice()), (StatusCode::OK, &b"%PDF-1.4 x"[..]));
    // A renderer that draws something that is no image gives no thumbnail either.
    *api.pdf_page.lock().expect("page") = Some(b"not a png".to_vec());
    let made = upload(&api, "two.pdf", b"%PDF-1.4 y").await;
    assert_eq!(made["thumb"], false);
}

#[tokio::test]
async fn a_pin_without_a_card_is_an_inbox_item_answered_in_place() {
    let api = Api::new();
    let (_, token) = setup(&api).await;
    let inbox = api.get("/v1/projects/DEMO/inbox").await;
    let item = inbox
        .as_array()
        .and_then(|a| a.iter().find(|i| i["kind"] == "pin"))
        .expect("a pin item")
        .clone();
    assert_eq!(
        (item["title"].as_str(), item["text"].as_str()),
        (Some("home.png"), Some("Which logo?"))
    );
    assert_eq!((item.get("ticket"), item.get("chain")), (None, None));
    let pin = item["pin"].as_str().expect("pin id");
    let project = item["project"].as_str().expect("project id");

    let (s, v) = api
        .call(
            Method::POST,
            &format!("/v1/projects/{project}/design-pins/{pin}/answer"),
            Some(json!({ "text": "The blue one." })),
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    let inbox = api.get("/v1/projects/DEMO/inbox").await;
    assert!(
        inbox
            .as_array()
            .expect("list")
            .iter()
            .all(|i| i["kind"] != "pin")
    );
    // The answer reaches the planner's plan.
    let (s, plan) = api
        .call_with(&token, Method::GET, "/v1/planner/plan", None)
        .await;
    assert_eq!(s, StatusCode::OK, "{plan}");
    assert!(
        plan["pins"]
            .as_array()
            .expect("pins")
            .iter()
            .any(|p| p["answer"] == "The blue one."),
        "{plan}"
    );
}
