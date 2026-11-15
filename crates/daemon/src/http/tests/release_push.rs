//! Push is a choice of the release, not of the merge mode: a local-mode project with a remote
//! pushes the base and the tag when the release asks for it, pushes nothing when it does not, names
//! a refused push by git's own line and runs again from the Push step, and a repository without a
//! remote offers no push at all. Real git, a bare repository as the remote.

use std::path::{Path, PathBuf};

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};

use super::Api;
use super::releases::{Env, env, finished_run, merge_card};
use crate::git::testing::git;

/// A bare repository next to the project's, added as `origin`.
fn remote_of(e: &Env) -> PathBuf {
    let remote = e._tmp.path().join("remote.git");
    git(e._tmp.path(), &["init", "--quiet", "--bare", "remote.git"]);
    git(
        &e.repo,
        &["remote", "add", "origin", &remote.to_string_lossy()],
    );
    remote
}

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

async fn release(api: &Api, body: Value) -> (StatusCode, Value) {
    api.call(Method::POST, "/v1/chains/auth/release", Some(body))
        .await
}

fn refs(remote: &Path) -> String {
    git(remote, &["for-each-ref", "--format=%(refname)"])
}

#[tokio::test]
async fn a_local_mode_release_with_push_lands_the_base_and_the_tag_in_the_remote() {
    let e = env().await;
    merge_card(&e, "First", "a.txt").await;
    let remote = remote_of(&e);

    let offer = e.api.get("/v1/chains/auth/release").await;
    assert_eq!(offer["remote"], json!(remote.to_string_lossy()), "{offer}");
    assert_eq!(offer["push"], json!(false), "no upstream: off by default");
    assert_eq!(offer["steps"], json!(["tag"]));

    let (s, body) = release(&e.api, json!({ "version": "2.0.0", "push": true })).await;
    assert_eq!(s, StatusCode::OK, "{body}");
    let run = finished_run(&e.api, "2.0.0").await;

    assert_eq!(states(&run), ["tag:done", "push:done"], "{run}");
    assert_eq!(
        run["steps"][1]["label"],
        json!("Pushed develop and v2.0.0 to origin"),
        "{run}"
    );
    assert_eq!(git(&remote, &["tag", "--list"]), "v2.0.0");
    assert_eq!(
        git(&remote, &["rev-parse", "develop"]),
        git(&e.repo, &["rev-parse", "develop"])
    );
}

#[tokio::test]
async fn a_release_without_push_sends_nothing_to_the_remote() {
    let e = env().await;
    merge_card(&e, "First", "a.txt").await;
    let remote = remote_of(&e);

    let (s, body) = release(&e.api, json!({ "version": "2.0.0", "push": false })).await;
    assert_eq!(s, StatusCode::OK, "{body}");
    let run = finished_run(&e.api, "2.0.0").await;

    assert_eq!(states(&run), ["tag:done"], "{run}");
    assert_eq!(
        git(&e.repo, &["tag", "--list"]),
        "v2.0.0",
        "the tag is local"
    );
    assert_eq!(refs(&remote), "", "nothing left the machine");
}

#[tokio::test]
async fn push_is_on_by_default_when_the_base_has_an_upstream_and_a_missing_choice_follows_it() {
    let e = env().await;
    merge_card(&e, "First", "a.txt").await;
    let remote = remote_of(&e);
    git(
        &e.repo,
        &["push", "--quiet", "--set-upstream", "origin", "develop"],
    );

    let offer = e.api.get("/v1/chains/auth/release").await;
    assert_eq!(offer["push"], json!(true), "{offer}");
    assert_eq!(offer["steps"], json!(["tag", "push"]));

    release(&e.api, json!({ "version": "2.0.0" })).await;
    let run = finished_run(&e.api, "2.0.0").await;
    assert_eq!(states(&run), ["tag:done", "push:done"], "{run}");
    assert_eq!(git(&remote, &["tag", "--list"]), "v2.0.0");
}

#[tokio::test]
async fn a_rejected_push_fails_the_step_with_gits_line_and_runs_again_from_push() {
    let e = env().await;
    merge_card(&e, "First", "a.txt").await;
    let remote = remote_of(&e);
    git(&e.repo, &["push", "--quiet", "origin", "develop"]);
    // The remote moves on: a commit of someone else lands on its develop.
    let other = e._tmp.path().join("other");
    git(
        e._tmp.path(),
        &["clone", "--quiet", &remote.to_string_lossy(), "other"],
    );
    git(&other, &["checkout", "--quiet", "develop"]);
    crate::git::testing::commit_file(&other, "theirs.txt", "x\n", "theirs");
    git(&other, &["push", "--quiet", "origin", "develop"]);
    let theirs = git(&remote, &["rev-parse", "develop"]);
    git(&e.repo, &["fetch", "--quiet", "origin"]);

    let (s, body) = release(&e.api, json!({ "version": "2.0.0", "push": true })).await;
    assert_eq!(s, StatusCode::OK, "{body}");
    let failed = finished_run(&e.api, "2.0.0").await;

    assert_eq!(states(&failed), ["tag:done", "push:failed"], "{failed}");
    let error = failed["error"].as_str().expect("error");
    assert_eq!(
        error, "Push failed: rejected: non-fast-forward; pull first",
        "{failed}"
    );
    assert_eq!(
        git(&remote, &["rev-parse", "develop"]),
        theirs,
        "the remote did not move"
    );
    assert_eq!(git(&e.repo, &["tag", "--list"]), "v2.0.0");

    // Their commit goes away (a force push of the person's own), so the push can pass now.
    git(
        &remote,
        &[
            "update-ref",
            "refs/heads/develop",
            &git(&e.repo, &["rev-parse", "origin/develop~1"]),
        ],
    );
    let (s, body) = release(&e.api, json!({ "version": "2.0.0", "resume": true })).await;
    assert_eq!(s, StatusCode::OK, "{body}");
    let run = finished_run(&e.api, "2.0.0").await;
    assert_eq!(states(&run), ["tag:done", "push:done"], "{run}");
    assert_eq!(
        run["steps"][0], failed["steps"][0],
        "the tag was not made again"
    );
    assert_eq!(git(&remote, &["tag", "--list"]), "v2.0.0");
    let chain = e.api.get("/v1/chains/auth").await;
    assert_eq!(chain["releases"][0]["state"], json!("released"));
}

#[tokio::test]
async fn a_repository_without_a_remote_offers_no_push_and_refuses_it_with_a_sentence() {
    let e = env().await;
    merge_card(&e, "First", "a.txt").await;

    let offer = e.api.get("/v1/chains/auth/release").await;
    assert_eq!(offer["remote"], Value::Null, "{offer}");
    assert_eq!(offer["push"], json!(false));
    assert_eq!(offer["steps"], json!(["tag"]));

    let (s, body) = release(&e.api, json!({ "version": "2.0.0", "push": true })).await;
    assert_eq!(s, StatusCode::CONFLICT, "{body}");
    assert!(
        body["error"].as_str().expect("error").contains("no remote"),
        "{body}"
    );
    assert!(
        git(&e.repo, &["tag", "--list"]).is_empty(),
        "nothing was tagged"
    );

    let (s, body) = release(&e.api, json!({ "version": "2.0.0", "push": false })).await;
    assert_eq!(s, StatusCode::OK, "{body}");
    let run = finished_run(&e.api, "2.0.0").await;
    assert_eq!(states(&run), ["tag:done"], "the tag stays local: {run}");
}

#[tokio::test]
async fn the_only_remote_is_used_when_none_is_called_origin_and_two_remotes_without_origin_are_none()
 {
    let e = env().await;
    merge_card(&e, "First", "a.txt").await;
    let remote = e._tmp.path().join("up.git");
    git(e._tmp.path(), &["init", "--quiet", "--bare", "up.git"]);
    git(
        &e.repo,
        &["remote", "add", "upstream", &remote.to_string_lossy()],
    );
    let offer = e.api.get("/v1/chains/auth/release").await;
    assert_eq!(offer["remote"], json!(remote.to_string_lossy()), "{offer}");
    release(&e.api, json!({ "version": "2.0.0", "push": true })).await;
    let run = finished_run(&e.api, "2.0.0").await;
    assert_eq!(
        run["steps"][1]["label"],
        json!("Pushed develop and v2.0.0 to upstream"),
        "{run}"
    );
    assert_eq!(git(&remote, &["tag", "--list"]), "v2.0.0");

    git(
        &e.repo,
        &["remote", "add", "second", &remote.to_string_lossy()],
    );
    let offer = e.api.get("/v1/chains/auth/release").await;
    assert_eq!(
        offer["remote"],
        Value::Null,
        "two remotes and none is origin: {offer}"
    );
}
