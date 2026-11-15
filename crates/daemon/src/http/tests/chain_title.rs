//! `POST /projects/{project}/chains` with a title and no name: the daemon derives the slug the
//! way the assistant does, and two epics with the same title do not clash.

use serde_json::json;

use super::Api;

#[tokio::test]
async fn a_title_without_a_name_makes_an_epic_with_the_derived_slug() {
    let api = Api::new();
    api.post("/v1/projects", json!({ "key": "DEMO", "name": "Demo" }))
        .await;
    let one = api
        .post(
            "/v1/projects/DEMO/chains",
            json!({ "title": "MIDI controllers" }),
        )
        .await;
    assert_eq!(one["chain"]["name"], json!("midi-controllers"), "{one}");
    assert_eq!(one["chain"]["title"], json!("MIDI controllers"), "{one}");
    assert_eq!(
        one["summary"]["display_title"],
        json!("MIDI controllers"),
        "{one}"
    );
    let two = api
        .post(
            "/v1/projects/DEMO/chains",
            json!({ "title": "MIDI controllers" }),
        )
        .await;
    assert_eq!(two["chain"]["name"], json!("midi-controllers-2"), "{two}");
    let cyr = api
        .post(
            "/v1/projects/DEMO/chains",
            json!({ "title": "Сховати проект зі списку" }),
        )
        .await;
    assert_eq!(
        cyr["chain"]["name"],
        json!("skhovaty-proekt-zi-spysku"),
        "{cyr}"
    );
}

#[tokio::test]
async fn neither_a_name_nor_a_title_is_refused() {
    let api = Api::new();
    api.post("/v1/projects", json!({ "key": "DEMO", "name": "Demo" }))
        .await;
    let (s, e) = api
        .call(
            axum::http::Method::POST,
            "/v1/projects/DEMO/chains",
            Some(json!({ "goal": "x" })),
        )
        .await;
    assert_eq!(s, axum::http::StatusCode::BAD_REQUEST, "{e}");
}
