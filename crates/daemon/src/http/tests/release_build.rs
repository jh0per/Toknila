//! The Build step of a release: the project's artifact command runs in a worktree at the tagged
//! commit once the tag is made, and a failing one leaves the run resumable at the Build alone.
//! The commands write their marks through `git config --add` (the repository's config is shared
//! with the release's worktree), never through a shell redirect, so they run on every platform.

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};

use super::releases::{env, finished_run, merge_card};
use crate::git::testing::git;

fn states(run: &Value) -> Vec<String> {
    run["steps"]
        .as_array()
        .expect("steps")
        .iter()
        .map(|s| {
            format!(
                "{}:{}",
                s["step"].as_str().expect("step"),
                s["state"].as_str().expect("state")
            )
        })
        .collect()
}

async fn set_artifact(e: &super::releases::Env, command: &str) {
    let (s, v) = e
        .api
        .call(
            Method::PATCH,
            "/v1/projects/DEMO",
            Some(json!({ "artifact_command": command })),
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{v}");
}

#[tokio::test]
async fn the_artifact_command_runs_after_the_tag_at_the_tagged_commit_and_the_run_is_released() {
    let e = env().await;
    merge_card(&e, "First", "a.txt").await;
    set_artifact(
        &e,
        "git config --add toknila.built {tag} && git describe --tags --exact-match HEAD",
    )
    .await;

    let offer = e.api.get("/v1/chains/auth/release").await;
    assert_eq!(offer["steps"], json!(["tag", "build"]), "{offer}");
    e.api
        .post("/v1/chains/auth/release", json!({ "version": "1.0.0" }))
        .await;
    let run = finished_run(&e.api, "1.0.0").await;

    assert_eq!(run["state"], json!("done"), "{run}");
    assert_eq!(states(&run), ["tag:done", "build:done"], "{run}");
    assert_eq!(
        run["steps"][1]["label"],
        json!("Build: git config --add"),
        "the step names the command's first words"
    );
    assert_eq!(offer["build"], json!("git config --add"));
    assert_eq!(
        git(&e.repo, &["config", "--get-all", "toknila.built"]),
        "v1.0.0"
    );
    let out = run["steps"][1]["output"].as_str().expect("output");
    assert!(out.contains("v1.0.0"), "the worktree was at the tag: {out}");
    let chain = e.api.get("/v1/chains/auth").await;
    assert_eq!(chain["releases"][0]["state"], json!("released"));
}

#[tokio::test]
async fn a_failing_artifact_command_leaves_the_tag_and_a_resume_runs_only_the_build() {
    let e = env().await;
    merge_card(&e, "First", "a.txt").await;
    // Fails (exit 1) until the repository's config says it may; then it leaves a mark.
    set_artifact(
        &e,
        "git config --get toknila.allow && git config --add toknila.built {tag}",
    )
    .await;
    e.api
        .post("/v1/chains/auth/release", json!({ "version": "2.0.0" }))
        .await;
    let failed = finished_run(&e.api, "2.0.0").await;
    assert_eq!(failed["state"], json!("failed"), "{failed}");
    assert_eq!(states(&failed), ["tag:done", "build:failed"], "{failed}");
    assert!(
        failed["error"]
            .as_str()
            .expect("error")
            .starts_with("Build failed"),
        "{failed}"
    );
    assert_eq!(git(&e.repo, &["tag", "--list"]), "v2.0.0", "the tag stays");

    git(&e.repo, &["config", "toknila.allow", "yes"]);
    e.api
        .post(
            "/v1/chains/auth/release",
            json!({ "version": "2.0.0", "resume": true }),
        )
        .await;
    let run = finished_run(&e.api, "2.0.0").await;
    assert_eq!(states(&run), ["tag:done", "build:done"], "{run}");
    assert_eq!(
        run["steps"][0], failed["steps"][0],
        "the tag step is the one journaled the first time"
    );
    assert_eq!(
        git(&e.repo, &["config", "--get-all", "toknila.built"]),
        "v2.0.0"
    );
    assert_eq!(git(&e.repo, &["tag", "--list"]), "v2.0.0");
    let chain = e.api.get("/v1/chains/auth").await;
    assert_eq!(chain["releases"][0]["state"], json!("released"));
}
