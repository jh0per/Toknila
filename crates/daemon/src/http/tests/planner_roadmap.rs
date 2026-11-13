//! The roadmap context of the assistant: a message about the roadmap lets the planner change the
//! top level, and add stories only to the epic it made after that message or one the message
//! names. A story of any other epic stays refused.

use axum::http::{Method, StatusCode};
use serde_json::json;

use super::Api;

async fn say(api: &Api, about: serde_json::Value, text: &str) {
    api.post(
        "/v1/projects/DEMO/conversation",
        json!({ "text": text, "about": about }),
    )
    .await;
}

/// The scripted planner of the instructions: "add X" is read and the two stories are proposed in
/// one question; the person's "давай" is one yes for the epic and its stories, built in the turn.
#[tokio::test]
async fn add_x_then_davai_leaves_an_epic_with_two_stories_and_no_open_the_epic_sentence() {
    let api = Api::new();
    api.two_projects().await;
    let token = api.planner_token("DEMO").await;
    let call = |m: Method, uri: String, body: serde_json::Value| {
        let (api, token) = (&api, token.clone());
        async move { api.call_with(&token, m, &uri, Some(body)).await }
    };
    let roadmap = json!({ "kind": "roadmap" });

    say(&api, roadmap.clone(), "добав сховати проект зі списку").await;
    let (s, q) = call(
        Method::POST,
        "/v1/planner/say".into(),
        json!({ "about": "roadmap", "text": "Дві сторі: поле hidden і фільтр списку. Робити?" }),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED, "{q}");
    say(&api, roadmap, "давай").await;

    let (s, epic) = call(
        Method::POST,
        "/v1/planner/chains".into(),
        json!({ "title": "Сховати проект зі списку", "goal": "g", "version": "0.3.4" }),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED, "{epic}");
    let name = epic["chain"]["name"].as_str().expect("name").to_owned();
    let add = format!("/v1/planner/chains/{name}/cards");
    let (s, first) = call(Method::POST, add.clone(), json!({ "title": "Поле hidden" })).await;
    assert_eq!(s, StatusCode::CREATED, "{first}");
    let key = first["ticket"]["key"].as_str().expect("key").to_owned();
    let (s, second) = call(
        Method::POST,
        add,
        json!({ "title": "Фільтр списку", "after": [key] }),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED, "{second}");
    let (s, _) = call(
        Method::POST,
        "/v1/planner/say".into(),
        json!({ "about": name, "text":
            "Епік: 2 сторі, перша «Поле hidden». Натисни «Run epic», щоб запустити їх по черзі." }),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED);

    let (s, chain) = api
        .call_with(
            &token,
            Method::GET,
            &format!("/v1/planner/chains/{name}"),
            None,
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{chain}");
    let cards = chain["cards"].as_array().expect("cards");
    let titles: Vec<_> = cards.iter().map(|c| c["ticket"]["title"].clone()).collect();
    assert_eq!(
        titles,
        [json!("Поле hidden"), json!("Фільтр списку")],
        "{chain}"
    );
    assert_eq!(
        cards[1]["after"],
        json!([cards[0]["ticket"]["key"]]),
        "{chain}"
    );
    let said: Vec<String> = messages(&api).await;
    assert!(
        said.iter()
            .all(|t| !t.to_lowercase().contains("open the epic")
                && !t.to_lowercase().contains("відкрий")),
        "{said:?}"
    );
}

async fn messages(api: &Api) -> Vec<String> {
    api.get("/v1/projects/DEMO/conversation").await["messages"]
        .as_array()
        .expect("messages")
        .iter()
        .filter(|m| m["author"] == "assistant")
        .map(|m| m["text"].as_str().unwrap_or_default().to_owned())
        .collect()
}

#[tokio::test]
async fn from_the_roadmap_stories_go_to_the_epic_made_in_the_turn_or_one_named_not_another() {
    let api = Api::new();
    api.two_projects().await;
    api.post("/v1/chains/plan/tickets", json!({ "title": "Old" }))
        .await;
    let token = api.planner_token("DEMO").await;
    let call = |m: Method, uri: String, body: serde_json::Value| {
        let (api, token) = (&api, token.clone());
        async move { api.call_with(&token, m, &uri, Some(body)).await }
    };

    say(&api, json!({ "kind": "roadmap" }), "add hiding a project").await;
    let (s, made) = call(
        Method::POST,
        "/v1/planner/chains".into(),
        json!({ "title": "Hide a project", "goal": "g" }),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED, "{made}");
    let name = made["chain"]["name"].as_str().expect("name").to_owned();
    let uri = format!("/v1/planner/chains/{name}/cards");

    // The epic made in this turn takes stories.
    let (s, one) = call(Method::POST, uri.clone(), json!({ "title": "One" })).await;
    assert_eq!(s, StatusCode::CREATED, "{one}");
    // Another epic does not, and neither does a story of it.
    let (s, body) = call(
        Method::POST,
        "/v1/planner/chains/plan/cards".into(),
        json!({ "title": "Sneaky" }),
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN, "{body}");
    assert!(
        body["error"]
            .as_str()
            .is_some_and(|e| e.contains("roadmap") && e.contains("plan")),
        "{body}"
    );
    let (s, body) = call(
        Method::PATCH,
        "/v1/planner/cards/DEMO-1".into(),
        json!({ "title": "Edited" }),
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN, "{body}");
    // A story of the epic made in the turn may be edited.
    let (s, body) = call(
        Method::PATCH,
        "/v1/planner/cards/DEMO-2".into(),
        json!({ "title": "One!" }),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{body}");

    // A new message about the roadmap is a new turn; naming an epic opens it.
    say(&api, json!({ "kind": "roadmap" }), "now more").await;
    let (s, body) = call(Method::POST, uri.clone(), json!({ "title": "Two" })).await;
    assert_eq!(s, StatusCode::FORBIDDEN, "{body}");
    say(&api, json!({ "kind": "roadmap" }), "add a story to plan").await;
    let (s, body) = call(
        Method::POST,
        "/v1/planner/chains/plan/cards".into(),
        json!({ "title": "Named" }),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED, "{body}");

    // Inside an epic's own context the rule does not apply.
    api.post(
        "/v1/projects/DEMO/conversation",
        json!({ "text": "more here", "about": { "kind": "chain", "name": name } }),
    )
    .await;
    let (s, body) = call(Method::POST, uri, json!({ "title": "Three" })).await;
    assert_eq!(s, StatusCode::CREATED, "{body}");
}
