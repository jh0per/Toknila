//! A chain's version and its end release through the full router and the real store: a new chain
//! carries the version and an end release, the version is edited while nothing is released, the
//! Finish node reads planned, ready and released, and a chain file carries the version both ways.

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};

use super::Api;
use super::releases::{env, finished_run, merge_card};

async fn patch(api: &Api, body: Value) -> (StatusCode, Value) {
    api.call(Method::PATCH, "/v1/chains/auth", Some(body)).await
}

async fn demo() -> Api {
    let api = Api::new();
    api.post("/v1/projects", json!({ "key": "DEMO", "name": "Demo" }))
        .await;
    api
}

#[tokio::test]
async fn a_new_chain_with_a_version_has_an_end_release_and_a_planned_finish() {
    let api = demo().await;
    let chain = api
        .post(
            "/v1/projects/DEMO/chains",
            json!({ "name": "auth", "goal": "Sign in with a passkey.", "version": "v0.3.0" }),
        )
        .await;
    assert_eq!(chain["chain"]["version"], json!("0.3.0"), "{chain}");
    assert_eq!(chain["releases"][0]["version"], json!("0.3.0"), "{chain}");
    assert_eq!(chain["summary"]["version"], json!("0.3.0"));
    assert_eq!(
        chain["summary"]["finish"],
        json!({ "version": "0.3.0", "state": "planned" })
    );
    // The sidebar's list carries the same version.
    let list = api.get("/v1/chains").await;
    assert_eq!(list[0]["summary"]["version"], json!("0.3.0"), "{list}");
}

#[tokio::test]
async fn a_chain_without_a_version_has_neither_and_a_bad_version_is_refused() {
    let api = demo().await;
    let plain = api
        .post("/v1/projects/DEMO/chains", json!({ "name": "plain" }))
        .await;
    assert!(plain["chain"].get("version").is_none(), "{plain}");
    assert!(plain["summary"].get("finish").is_none(), "{plain}");
    assert_eq!(plain["releases"], json!([]));
    let (s, e) = api
        .call(
            Method::POST,
            "/v1/projects/DEMO/chains",
            Some(json!({ "name": "bad", "version": "soon" })),
        )
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{e}");
}

#[tokio::test]
async fn the_epic_is_edited_in_place_and_a_taken_name_is_refused() {
    let api = demo().await;
    api.post("/v1/projects/DEMO/chains", json!({ "name": "auth" }))
        .await;
    api.post("/v1/projects/DEMO/chains", json!({ "name": "other" }))
        .await;
    let (s, c) = patch(
        &api,
        json!({ "name": "login", "goal": "  Sign in.\n\nMore.  ", "version": "0.4.0" }),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{c}");
    assert_eq!(c["chain"]["name"], json!("login"));
    assert_eq!(c["chain"]["goal"], json!("Sign in.\n\nMore."));
    assert_eq!(c["summary"]["finish"]["version"], json!("0.4.0"), "{c}");
    assert_eq!(
        c["releases"][0]["version"],
        json!("0.4.0"),
        "a release was made"
    );
    // A version change renames the planned release instead of adding one.
    let (s, c) = api
        .call(
            Method::PATCH,
            "/v1/chains/login",
            Some(json!({ "version": "0.5.0" })),
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{c}");
    assert_eq!(c["releases"].as_array().expect("releases").len(), 1);
    assert_eq!(c["releases"][0]["version"], json!("0.5.0"));
    let (s, e) = api
        .call(
            Method::PATCH,
            "/v1/chains/login",
            Some(json!({ "name": "other" })),
        )
        .await;
    assert_eq!(s, StatusCode::CONFLICT, "{e}");
    let (s, e) = api
        .call(
            Method::PATCH,
            "/v1/chains/login",
            Some(json!({ "name": "  " })),
        )
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{e}");
}

#[tokio::test]
async fn the_version_is_written_to_the_chain_file_and_an_import_reads_it_back() {
    let api = demo().await;
    api.post(
        "/v1/projects/DEMO/chains",
        json!({ "name": "auth", "version": "0.3.0" }),
    )
    .await;
    api.post("/v1/chains/auth/tickets", json!({ "title": "A" }))
        .await;
    let file = api.get("/v1/chains/auth/export").await;
    let toml = file["toml"].as_str().expect("toml").to_owned();
    assert!(toml.contains("version = \"0.3.0\""), "{toml}");
    let again = toml.replace("name = \"auth\"", "name = \"copy\"");
    let imported = api
        .post("/v1/projects/DEMO/import", json!({ "toml": again }))
        .await;
    assert_eq!(imported["chain"]["version"], json!("0.3.0"), "{imported}");
    assert_eq!(imported["releases"][0]["version"], json!("0.3.0"));
    // A file with a version and no release gets one that waits for the last card.
    let bare = "[chain]\nname = \"bare\"\nversion = \"1.2.0\"\n\n[[ticket]]\nid = \"A\"\n";
    let bare = api
        .post("/v1/projects/DEMO/import", json!({ "toml": bare }))
        .await;
    assert_eq!(bare["releases"][0]["version"], json!("1.2.0"), "{bare}");
    assert_eq!(bare["edges"].as_array().expect("edges").len(), 1, "{bare}");
    let (s, e) = api
        .call(
            Method::POST,
            "/v1/projects/DEMO/import",
            Some(json!({ "toml": "[chain]\nname = \"x\"\nversion = \"soon\"\n" })),
        )
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{e}");
}

#[tokio::test]
async fn a_project_suggests_the_next_patch_of_its_newest_tag() {
    let none = demo().await;
    let s = none.get("/v1/projects/DEMO/next-version").await;
    assert_eq!(s, json!({ "version": "0.1.0", "last_tag": null }));

    let e = env().await;
    crate::git::testing::git(&e.repo, &["tag", "v0.2.4"]);
    let s = e.api.get("/v1/projects/DEMO/next-version").await;
    assert_eq!(s, json!({ "version": "0.2.5", "last_tag": "v0.2.4" }));
}

#[tokio::test]
async fn the_finish_goes_planned_ready_released_with_the_tag() {
    let e = env().await;
    let (s, c) = patch(&e.api, json!({ "version": "0.3.0" })).await;
    assert_eq!(s, StatusCode::OK, "{c}");
    e.api
        .post("/v1/chains/auth/tickets", json!({ "title": "Open" }))
        .await;
    let c = e.api.get("/v1/chains/auth").await;
    assert_eq!(c["summary"]["finish"]["state"], json!("planned"), "{c}");
    e.api
        .post("/v1/tickets/DEMO-1/actions/cancel", json!({ "text": "no" }))
        .await;
    merge_card(&e, "Second", "b.txt").await;
    let c = e.api.get("/v1/chains/auth").await;
    assert_eq!(c["summary"]["finish"]["state"], json!("ready"), "{c}");

    e.api
        .post("/v1/chains/auth/release", json!({ "version": "0.3.0" }))
        .await;
    let run = finished_run(&e.api, "0.3.0").await;
    assert_eq!(run["state"], json!("done"), "{run}");
    let c = e.api.get("/v1/chains/auth").await;
    let finish = &c["summary"]["finish"];
    assert_eq!(finish["state"], json!("released"), "{c}");
    assert_eq!(finish["tag"], json!("v0.3.0"));
    assert!(finish["released_at"].is_number(), "{finish}");
    // Released: the version cannot change any more.
    let (s, e) = patch(&e.api, json!({ "version": "0.4.0" })).await;
    assert_eq!(s, StatusCode::CONFLICT, "{e}");
}
