//! An epic that shipped inside another epic's tag is released with it, and a version belongs to one
//! epic: through the full router with a real repository. A tag carries every finished epic whose
//! merges it contains; a person can say it for an old tag; one planned version per project.

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};

use super::releases::{Env, env, finished_run};
use crate::git::testing::{commit_file, git};
use std::path::PathBuf;

/// A chain of the project with a planned version.
async fn epic(e: &Env, name: &str, version: &str) {
    let (s, v) = e
        .api
        .call(
            Method::POST,
            "/v1/projects/DEMO/chains",
            Some(json!({ "name": name, "version": version })),
        )
        .await;
    assert_eq!(s, StatusCode::CREATED, "{v}");
}

/// A story of `chain` with one commit on its ship, handed in but not accepted.
async fn story(e: &Env, chain: &str, title: &str, file: &str) -> String {
    let t = e
        .api
        .post(
            &format!("/v1/chains/{chain}/tickets"),
            json!({ "title": title }),
        )
        .await;
    let key = t["ticket"]["key"].as_str().expect("key").to_owned();
    let d = e.api.get(&format!("/v1/tickets/{key}")).await;
    let ship = PathBuf::from(d["ship"]["path"].as_str().expect("ship"));
    commit_file(&ship, file, "x\n", title);
    e.api
        .post(
            &format!("/v1/tickets/{key}/events"),
            json!({ "event": "finished", "summary": toknila_core::handin::from_plain("done") }),
        )
        .await;
    key
}

async fn accept(e: &Env, key: &str) {
    e.api
        .post(&format!("/v1/tickets/{key}/actions/accept"), json!({}))
        .await;
}

async fn merged(e: &Env, chain: &str, title: &str, file: &str) -> String {
    let key = story(e, chain, title, file).await;
    accept(e, &key).await;
    key
}

async fn mark(e: &Env, chain: &str, tag: &str) -> (StatusCode, Value) {
    e.api
        .call(
            Method::POST,
            &format!("/v1/chains/{chain}/release/mark"),
            Some(json!({ "tag": tag })),
        )
        .await
}

async fn release_auth(e: &Env, version: &str) {
    let (s, v) = e
        .api
        .call(
            Method::POST,
            "/v1/chains/auth/release",
            Some(json!({ "version": version, "push": false })),
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    let run = finished_run(&e.api, version).await;
    assert_eq!(run["state"], json!("done"), "{run}");
}

#[tokio::test]
async fn a_tag_carries_the_finished_epics_it_contains_and_leaves_the_others() {
    let e = env().await;
    epic(&e, "before", "0.3.4").await;
    epic(&e, "open", "0.3.5").await;
    epic(&e, "after", "0.3.6").await;
    merged(&e, "auth", "Own", "a.txt").await;
    let b1 = merged(&e, "before", "Done before", "b.txt").await;
    let b2 = merged(&e, "before", "Done before too", "b2.txt").await;
    merged(&e, "open", "Merged", "c.txt").await;
    let still = story(&e, "open", "Not merged", "c2.txt").await;
    // The last story of `after` is handed in before the tag and merged after it.
    merged(&e, "after", "Early", "d.txt").await;
    let late = story(&e, "after", "Late", "d2.txt").await;

    release_auth(&e, "0.3.3").await;
    accept(&e, &late).await;

    let before = e.api.get("/v1/chains/before").await;
    let r = &before["releases"][0];
    assert_eq!(r["state"], json!("released"), "{before}");
    assert_eq!(r["tag"], json!("v0.3.3"));
    assert_eq!(r["version"], json!("0.3.3"), "planned higher: the tag's");
    assert_eq!(r["released_with"], json!({ "tag": "v0.3.3", "by": "auth" }));
    assert_eq!(before["chain"]["version"], json!("0.3.3"));
    assert_eq!(
        before["summary"]["finish"]["released_with"],
        json!({ "tag": "v0.3.3", "by": "auth" }),
        "{before}"
    );
    for key in [&b1, &b2] {
        let t = e.api.get(&format!("/v1/tickets/{key}")).await;
        assert_eq!(t["ticket"]["status"], json!("released"), "{key}");
    }
    let events = e.api.get("/v1/events").await.to_string();
    assert!(events.contains("state_changed"), "journaled: {events}");

    // An epic with an open story, and one merged after the tag, stay as they were.
    for (chain, version) in [("open", "0.3.5"), ("after", "0.3.6")] {
        let c = e.api.get(&format!("/v1/chains/{chain}")).await;
        assert_eq!(c["releases"][0]["state"], json!("planned"), "{chain}: {c}");
        assert_eq!(c["releases"][0]["version"], json!(version));
        assert!(c["releases"][0].get("released_with").is_none(), "{c}");
    }
    let t = e.api.get(&format!("/v1/tickets/{still}")).await;
    assert_ne!(t["ticket"]["status"], json!("released"));
    let t = e.api.get(&format!("/v1/tickets/{late}")).await;
    assert_eq!(t["ticket"]["status"], json!("merged"), "after the tag");
}

#[tokio::test]
async fn release_steps_answer_to_the_epics_name_and_a_shared_version_lists_both_epics() {
    let e = env().await;
    epic(&e, "before", "0.3.4").await;
    merged(&e, "auth", "Own", "a.txt").await;
    merged(&e, "before", "Done", "b.txt").await;
    release_auth(&e, "0.3.3").await;

    // `before` now carries 0.3.3 too, yet `0.3.3` still means the release that was cut.
    let by_version = e.api.get("/v1/releases/0.3.3/run").await;
    assert_eq!(by_version["tag"], json!("v0.3.3"));
    let by_name = e.api.get("/v1/releases/auth/run").await;
    assert_eq!(by_name, by_version);
    let (s, v) = e
        .api
        .call(Method::GET, "/v1/releases/before/run", None)
        .await;
    assert_eq!(s, StatusCode::NOT_FOUND, "{v}");
    assert!(
        v["error"]
            .as_str()
            .is_some_and(|m| m.contains("a release run of")),
        "no run of its own: {v}"
    );
}

#[tokio::test]
async fn old_data_with_one_version_in_two_epics_names_both_in_the_error() {
    let e = env().await;
    // Two epics that plan one version cannot be made any more through the epic's version; the
    // milestone route of old data still can.
    for name in ["one", "two"] {
        e.api
            .post("/v1/projects/DEMO/chains", json!({ "name": name }))
            .await;
        e.api
            .post(
                &format!("/v1/chains/{name}/releases"),
                json!({ "version": "0.3.4" }),
            )
            .await;
    }
    let (s, v) = e
        .api
        .call(Method::GET, "/v1/releases/0.3.4/run", None)
        .await;
    assert_eq!(s, StatusCode::CONFLICT, "{v}");
    let text = v["error"].as_str().expect("error");
    assert!(
        text.contains("2 releases have version 0.3.4")
            && text.contains("one")
            && text.contains("two"),
        "{text}"
    );
}

#[tokio::test]
async fn a_person_marks_an_epic_released_with_an_old_tag() {
    let e = env().await;
    epic(&e, "stale", "0.3.4").await;
    let a = merged(&e, "stale", "Done", "a.txt").await;
    git(&e.repo, &["tag", "v0.3.3"]);

    let (s, v) = mark(&e, "stale", "v0.3.3").await;
    assert_eq!(s, StatusCode::OK, "{v}");
    let r = &v["releases"][0];
    assert_eq!(r["state"], json!("released"));
    assert_eq!(r["tag"], json!("v0.3.3"));
    assert_eq!(r["version"], json!("0.3.3"));
    assert_eq!(r["released_with"], json!({ "tag": "v0.3.3" }), "{v}");
    let t = e.api.get(&format!("/v1/tickets/{a}")).await;
    assert_eq!(t["ticket"]["status"], json!("released"));
    // The version 0.3.4 is free again.
    epic(&e, "next", "0.3.4").await;
}

#[tokio::test]
async fn marking_with_a_tag_that_does_not_exist_or_does_not_contain_the_merges_is_refused() {
    let e = env().await;
    epic(&e, "stale", "0.3.4").await;
    merged(&e, "stale", "First", "a.txt").await;

    let (s, v) = mark(&e, "stale", "v9.9.9").await;
    assert_eq!(s, StatusCode::CONFLICT, "{v}");
    assert_eq!(
        v["error"],
        json!("the tag v9.9.9 does not exist in the repository"),
        "{v}"
    );

    // A tag made before the merge does not contain it.
    git(&e.repo, &["tag", "v0.0.1", "develop~1"]);
    let (s, v) = mark(&e, "stale", "v0.0.1").await;
    assert_eq!(s, StatusCode::CONFLICT, "{v}");
    assert_eq!(
        v["error"],
        json!("the tag v0.0.1 does not contain the merges of epic stale"),
        "{v}"
    );
    let c = e.api.get("/v1/chains/stale").await;
    assert_eq!(c["releases"][0]["state"], json!("planned"), "{c}");

    // An epic with an open story is refused by its state, tag or not.
    epic(&e, "busy", "0.3.5").await;
    story(&e, "busy", "Open", "b.txt").await;
    git(&e.repo, &["tag", "v0.3.3"]);
    let (s, v) = mark(&e, "busy", "v0.3.3").await;
    assert_eq!(s, StatusCode::CONFLICT, "{v}");
    assert!(
        v["error"]
            .as_str()
            .is_some_and(|m| m.contains("not finished")),
        "{v}"
    );
}

#[tokio::test]
async fn one_planned_version_per_project_in_the_api() {
    let e = env().await;
    epic(&e, "phase-10", "0.3.4").await;

    let (s, v) = e
        .api
        .call(
            Method::POST,
            "/v1/projects/DEMO/chains",
            Some(json!({ "name": "phase-11", "version": "v0.3.4" })),
        )
        .await;
    assert_eq!(s, StatusCode::CONFLICT, "{v}");
    assert_eq!(
        v["error"],
        json!("0.3.4 is planned by phase-10; pick another version or move that one"),
        "{v}"
    );
    epic(&e, "phase-11", "0.3.5").await;
    let (s, v) = e
        .api
        .call(
            Method::PATCH,
            "/v1/chains/phase-11",
            Some(json!({ "version": "0.3.4" })),
        )
        .await;
    assert_eq!(s, StatusCode::CONFLICT, "{v}");
    assert_eq!(
        v["error"],
        json!("0.3.4 is planned by phase-10; pick another version or move that one"),
        "{v}"
    );
    // Saving an epic with the version it already has is not a clash.
    let (s, v) = e
        .api
        .call(
            Method::PATCH,
            "/v1/chains/phase-11",
            Some(json!({ "version": "0.3.5", "goal": "g" })),
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{v}");
}

#[tokio::test]
async fn the_next_version_skips_the_planned_ones() {
    let e = env().await;
    git(&e.repo, &["tag", "v0.3.3"]);
    epic(&e, "phase-10", "0.3.4").await;
    let v = e.api.get("/v1/projects/DEMO/next-version").await;
    assert_eq!(v["version"], json!("0.3.5"), "{v}");
}
