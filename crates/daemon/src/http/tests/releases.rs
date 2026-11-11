//! A finished chain becomes a release: through the full router, with the real git shipyard and
//! release run on a temporary repository. Nothing is tagged before the person's click, the steps
//! run one at a time, a failure stops at the step it names, and the milestone reads released with
//! the tag.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};

use super::Api;
use super::ships::{project, repo};
use crate::git::testing::{commit_file, git};
use crate::shipyard::GitShipyard;

pub(super) struct Env {
    pub(super) _tmp: tempfile::TempDir,
    pub(super) repo: PathBuf,
    pub(super) api: Api,
}

pub(super) async fn env() -> Env {
    let tmp = tempfile::tempdir().expect("tmp");
    let repo = repo(tmp.path());
    let api = Api::with_yard(Some(Arc::new(GitShipyard::new(tmp.path().join("ships")))));
    project(&api, &repo).await;
    Env {
        _tmp: tmp,
        repo,
        api,
    }
}

/// Adds a card, commits a file on its ship and accepts it: the card is Merged.
pub(super) async fn merge_card(e: &Env, title: &str, file: &str) -> String {
    let t = e
        .api
        .post("/v1/chains/auth/tickets", json!({ "title": title }))
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
    e.api
        .post(&format!("/v1/tickets/{key}/actions/accept"), json!({}))
        .await;
    key
}

async fn settings(e: &Env, check: &str, bump: &str) {
    let (s, v) = e
        .api
        .call(
            Method::PATCH,
            "/v1/projects/DEMO",
            Some(json!({ "check_command": check, "bump_command": bump })),
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{v}");
}

/// Waits until the last run of `version` is no longer running and returns it.
pub(super) async fn finished_run(api: &Api, version: &str) -> Value {
    for _ in 0..600 {
        let (s, run) = api
            .call(Method::GET, &format!("/v1/releases/{version}/run"), None)
            .await;
        if s == StatusCode::OK && run["state"] != "running" {
            return run;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("the release run did not finish");
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

#[tokio::test]
async fn a_chain_with_every_card_merged_offers_release_and_a_chain_with_an_open_card_does_not() {
    let e = env().await;
    merge_card(&e, "First", "a.txt").await;
    e.api
        .post(
            "/v1/chains/auth/tickets",
            json!({ "title": "Open", "autostart": false }),
        )
        .await;

    let chain = e.api.get("/v1/chains/auth").await;
    assert_eq!(chain["summary"]["releasable"], json!(false));
    let offer = e.api.get("/v1/chains/auth/release").await;
    assert_eq!(offer["releasable"], json!(false));
    let (s, body) = e
        .api
        .call(
            Method::POST,
            "/v1/chains/auth/release",
            Some(json!({ "version": "0.1.0" })),
        )
        .await;
    assert_eq!(s, StatusCode::CONFLICT, "{body}");
    assert!(
        git(&e.repo, &["tag", "--list"]).is_empty(),
        "nothing is tagged"
    );

    e.api
        .post(
            "/v1/tickets/DEMO-2/actions/cancel",
            json!({ "text": "not needed" }),
        )
        .await;
    let chain = e.api.get("/v1/chains/auth").await;
    assert_eq!(chain["summary"]["releasable"], json!(true), "{chain}");
}

#[tokio::test]
async fn release_runs_the_check_the_bump_and_the_tag_in_order_and_marks_the_release_released() {
    let e = env().await;
    merge_card(&e, "First", "a.txt").await;
    settings(&e, "echo checked", "echo {version}> VERSION").await;
    git(&e.repo, &["tag", "v1.4.2"]);

    let offer = e.api.get("/v1/chains/auth/release").await;
    assert_eq!(offer["releasable"], json!(true));
    assert_eq!(
        offer["version"],
        json!("1.4.3"),
        "the next patch of the last tag"
    );
    assert_eq!(offer["last_tag"], json!("v1.4.2"));
    assert_eq!(offer["steps"], json!(["check", "bump", "tag"]));
    assert_eq!(offer["base"], json!("develop"));
    assert_eq!(
        git(&e.repo, &["tag", "--list"]),
        "v1.4.2",
        "an offer changes nothing"
    );

    let detail = e
        .api
        .post("/v1/chains/auth/release", json!({ "version": "1.4.3" }))
        .await;
    assert_eq!(detail["releases"][0]["version"], json!("1.4.3"));
    let run = finished_run(&e.api, "1.4.3").await;

    assert_eq!(run["state"], json!("done"), "{run}");
    assert_eq!(states(&run), ["check:done", "bump:done", "tag:done"]);
    assert!(
        run["steps"][0]["output"]
            .as_str()
            .expect("out")
            .contains("checked")
    );
    assert_eq!(run["tag"], json!("v1.4.3"));
    assert_eq!(git(&e.repo, &["tag", "--list", "v1.4.3"]), "v1.4.3");
    assert_eq!(
        git(&e.repo, &["rev-list", "-n", "1", "v1.4.3"]),
        git(&e.repo, &["rev-parse", "develop"])
    );
    assert_eq!(
        git(&e.repo, &["show", "develop:VERSION"]),
        "1.4.3",
        "the bump is on the base branch"
    );
    assert_eq!(
        git(&e.repo, &["log", "-1", "--format=%s", "develop"]),
        "Release 1.4.3"
    );
    assert!(
        e.repo.join("VERSION").exists(),
        "the person's checkout moved with the branch"
    );

    let chain = e.api.get("/v1/chains/auth").await;
    assert_eq!(chain["releases"][0]["state"], json!("released"));
    assert_eq!(chain["releases"][0]["tag"], json!("v1.4.3"));
    assert_eq!(chain["tickets"][0]["status"], json!("released"));
    assert_eq!(
        chain["summary"]["releasable"],
        json!(false),
        "released work is not offered again"
    );
}

#[tokio::test]
async fn a_failing_step_stops_the_run_names_the_step_and_tags_nothing_and_a_second_try_goes_through()
 {
    let e = env().await;
    merge_card(&e, "First", "a.txt").await;
    settings(&e, "echo broken && exit 3", "echo {version}> VERSION").await;
    let before = git(&e.repo, &["rev-parse", "develop"]);

    e.api
        .post("/v1/chains/auth/release", json!({ "version": "0.1.0" }))
        .await;
    let run = finished_run(&e.api, "0.1.0").await;

    assert_eq!(run["state"], json!("failed"), "{run}");
    assert_eq!(
        states(&run),
        ["check:failed", "bump:pending", "tag:pending"]
    );
    assert!(
        run["error"]
            .as_str()
            .expect("error")
            .starts_with("Check failed"),
        "{run}"
    );
    assert!(
        run["steps"][0]["output"]
            .as_str()
            .expect("out")
            .contains("broken")
    );
    assert!(
        git(&e.repo, &["tag", "--list"]).is_empty(),
        "no tag after a failed check"
    );
    assert_eq!(
        git(&e.repo, &["rev-parse", "develop"]),
        before,
        "the base did not move"
    );
    let chain = e.api.get("/v1/chains/auth").await;
    assert_eq!(chain["releases"][0]["state"], json!("cut"));

    settings(&e, "echo fixed", "echo {version}> VERSION").await;
    e.api
        .post("/v1/chains/auth/release", json!({ "version": "0.1.0" }))
        .await;
    let run = finished_run(&e.api, "0.1.0").await;
    assert_eq!(run["state"], json!("done"), "{run}");
    assert_eq!(git(&e.repo, &["tag", "--list"]), "v0.1.0");
    let chain = e.api.get("/v1/chains/auth").await;
    assert_eq!(chain["releases"][0]["state"], json!("released"));
}

#[tokio::test]
async fn a_project_without_a_check_or_a_bump_command_just_tags() {
    let e = env().await;
    merge_card(&e, "First", "a.txt").await;
    let offer = e.api.get("/v1/chains/auth/release").await;
    assert_eq!(offer["version"], json!("0.1.0"));
    assert_eq!(offer["steps"], json!(["tag"]));
    e.api
        .post("/v1/chains/auth/release", json!({ "version": "v0.1.0" }))
        .await;
    let run = finished_run(&e.api, "0.1.0").await;
    assert_eq!(states(&run), ["tag:done"], "{run}");
    assert_eq!(git(&e.repo, &["tag", "--list"]), "v0.1.0");
}

#[tokio::test]
async fn a_version_that_is_not_one_is_refused_before_anything_happens() {
    let e = env().await;
    merge_card(&e, "First", "a.txt").await;
    let (s, body) = e
        .api
        .call(
            Method::POST,
            "/v1/chains/auth/release",
            Some(json!({ "version": "1.0; rm -rf" })),
        )
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{body}");
    let chain = e.api.get("/v1/chains/auth").await;
    assert_eq!(chain["releases"], json!([]), "no milestone was made");
}

#[tokio::test]
async fn a_project_that_merges_by_pull_request_also_pushes_the_base_and_the_tag() {
    let e = env().await;
    merge_card(&e, "First", "a.txt").await;
    let remote = e._tmp.path().join("remote.git");
    git(e._tmp.path(), &["init", "--quiet", "--bare", "remote.git"]);
    git(
        &e.repo,
        &["remote", "add", "origin", &remote.to_string_lossy()],
    );
    let (s, v) = e
        .api
        .call(
            Method::PATCH,
            "/v1/projects/DEMO",
            Some(json!({ "merge_mode": "github" })),
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    let offer = e.api.get("/v1/chains/auth/release").await;
    assert_eq!(offer["steps"], json!(["tag", "push"]));

    e.api
        .post("/v1/chains/auth/release", json!({ "version": "2.0.0" }))
        .await;
    let run = finished_run(&e.api, "2.0.0").await;

    assert_eq!(states(&run), ["tag:done", "push:done"], "{run}");
    assert_eq!(git(&remote, &["tag", "--list"]), "v2.0.0");
    assert_eq!(
        git(&remote, &["rev-parse", "develop"]),
        git(&e.repo, &["rev-parse", "develop"]),
        "the base branch is on the remote"
    );
}

#[tokio::test]
async fn a_failing_push_is_the_step_that_is_named() {
    let e = env().await;
    merge_card(&e, "First", "a.txt").await;
    let (s, _) = e
        .api
        .call(
            Method::PATCH,
            "/v1/projects/DEMO",
            Some(json!({ "merge_mode": "github" })),
        )
        .await;
    assert_eq!(s, StatusCode::OK);
    e.api
        .post("/v1/chains/auth/release", json!({ "version": "2.0.0" }))
        .await;
    let run = finished_run(&e.api, "2.0.0").await;
    assert_eq!(states(&run), ["tag:done", "push:failed"], "{run}");
    assert!(
        run["error"]
            .as_str()
            .expect("error")
            .starts_with("Push failed"),
        "{run}"
    );
    let chain = e.api.get("/v1/chains/auth").await;
    assert_eq!(
        chain["releases"][0]["state"],
        json!("cut"),
        "not released when the push failed"
    );
}

#[tokio::test]
async fn the_steps_of_a_run_are_in_the_journal_and_are_still_there_after_a_restart() {
    let e = env().await;
    merge_card(&e, "First", "a.txt").await;
    settings(&e, "echo checked", "echo {version}> VERSION").await;
    e.api
        .post("/v1/chains/auth/release", json!({ "version": "1.0.0" }))
        .await;
    let before = finished_run(&e.api, "1.0.0").await;
    assert_eq!(before["state"], json!("done"), "{before}");

    let Env { _tmp, repo, api } = e;
    let kinds: Vec<String> = api
        .get("/v1/events")
        .await
        .as_array()
        .expect("events")
        .iter()
        .filter(|e| e["entity"] == json!("release_run"))
        .map(|e| e["kind"].as_str().expect("kind").to_owned())
        .collect();
    assert_eq!(
        kinds,
        [
            "run_started",
            "step_started",
            "step_passed",
            "step_started",
            "step_passed",
            "step_started",
            "step_passed",
            "run_done"
        ],
        "every step is a journaled event"
    );

    let api = api.restart();
    let (s, after) = api.call(Method::GET, "/v1/releases/1.0.0/run", None).await;
    assert_eq!(s, StatusCode::OK, "{after}");
    assert_eq!(after, before, "the run reads the same after a restart");
    let offer = api.get("/v1/chains/auth").await;
    assert_eq!(offer["releases"][0]["state"], json!("released"));
    assert_eq!(git(&repo, &["tag", "--list"]), "v1.0.0");
}

/// What a daemon that died in the middle of a run left in the journal.
async fn left_by_a_dead_daemon(api: &Api, release: &str, entries: Vec<Entry>) {
    let id: toknila_core::ReleaseId = release.parse().expect("release id");
    api.store
        .write(
            toknila_core::Actor::System,
            crate::server::now(),
            move |tx| -> Result<(), crate::store::StoreError> {
                for e in &entries {
                    tx.journal_release_run(id, e)?;
                }
                Ok(())
            },
        )
        .await
        .expect("journal");
}

use crate::store::release_runs::Entry;
use toknila_core::release_run::ReleaseStep::{Bump, Check, Tag};

#[tokio::test]
async fn a_run_cut_off_by_a_restart_is_interrupted_at_its_step_and_runs_again_from_there() {
    let e = env().await;
    merge_card(&e, "First", "a.txt").await;
    settings(&e, "echo checked-again", "echo {version}> VERSION").await;
    let chain = e
        .api
        .post("/v1/chains/auth/releases", json!({ "version": "1.0.0" }))
        .await;
    let id = chain["releases"][0]["id"].as_str().expect("id").to_owned();
    left_by_a_dead_daemon(
        &e.api,
        &id,
        vec![
            Entry::RunStarted {
                version: "1.0.0".to_owned(),
                tag: "v1.0.0".to_owned(),
                steps: vec![Check, Bump, Tag],
            },
            Entry::StepStarted { step: Check },
            Entry::StepPassed {
                step: Check,
                output: "checked-before the restart".to_owned(),
            },
            Entry::StepStarted { step: Bump },
        ],
    )
    .await;
    let (s, run) = e
        .api
        .call(Method::GET, "/v1/releases/1.0.0/run", None)
        .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(run["state"], json!("running"), "while nothing restarted");

    let Env { _tmp, repo, api } = e;
    let api = api.restart();
    crate::recovery::recover(&api.store).await.expect("recover");
    let run = api.get("/v1/releases/1.0.0/run").await;
    assert_eq!(run["state"], json!("interrupted"), "{run}");
    assert_eq!(run["resume_from"], json!("bump"));
    assert_eq!(
        states(&run),
        ["check:done", "bump:interrupted", "tag:pending"]
    );

    // A fresh start is not the same as running it again; asking to resume a run that is not
    // interrupted is refused.
    let (s, body) = api
        .call(
            Method::POST,
            "/v1/chains/auth/release",
            Some(json!({ "version": "1.0.0", "resume": true })),
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{body}");
    let run = finished_run(&api, "1.0.0").await;
    assert_eq!(run["state"], json!("done"), "{run}");
    assert_eq!(states(&run), ["check:done", "bump:done", "tag:done"]);
    assert!(
        run["steps"][0]["output"]
            .as_str()
            .expect("out")
            .contains("checked-before the restart"),
        "the check that passed was not run again: {run}"
    );
    assert_eq!(git(&repo, &["tag", "--list"]), "v1.0.0");
    assert_eq!(git(&repo, &["show", "develop:VERSION"]), "1.0.0");

    let (s, body) = api
        .call(
            Method::POST,
            "/v1/chains/auth/release",
            Some(json!({ "version": "1.0.0", "resume": true })),
        )
        .await;
    assert_eq!(s, StatusCode::CONFLICT, "{body}");
}

#[tokio::test]
async fn running_again_after_a_failed_push_does_not_redo_the_tag() {
    let e = env().await;
    merge_card(&e, "First", "a.txt").await;
    let (s, _) = e
        .api
        .call(
            Method::PATCH,
            "/v1/projects/DEMO",
            Some(json!({ "merge_mode": "github" })),
        )
        .await;
    assert_eq!(s, StatusCode::OK);
    e.api
        .post("/v1/chains/auth/release", json!({ "version": "2.0.0" }))
        .await;
    let failed = finished_run(&e.api, "2.0.0").await;
    assert_eq!(states(&failed), ["tag:done", "push:failed"], "{failed}");

    let remote = e._tmp.path().join("remote.git");
    git(e._tmp.path(), &["init", "--quiet", "--bare", "remote.git"]);
    git(
        &e.repo,
        &["remote", "add", "origin", &remote.to_string_lossy()],
    );
    e.api
        .post(
            "/v1/chains/auth/release",
            json!({ "version": "2.0.0", "resume": true }),
        )
        .await;
    let run = finished_run(&e.api, "2.0.0").await;
    assert_eq!(states(&run), ["tag:done", "push:done"], "{run}");
    assert_eq!(
        run["steps"][0], failed["steps"][0],
        "the tag step is the one that was journaled the first time"
    );
    assert_eq!(git(&remote, &["tag", "--list"]), "v2.0.0");
    let chain = e.api.get("/v1/chains/auth").await;
    assert_eq!(chain["releases"][0]["state"], json!("released"));
}

#[tokio::test]
async fn a_released_chain_offers_release_again_after_a_new_card_merges_with_the_next_version() {
    let e = env().await;
    merge_card(&e, "First", "a.txt").await;
    e.api
        .post("/v1/chains/auth/release", json!({ "version": "0.1.0" }))
        .await;
    finished_run(&e.api, "0.1.0").await;
    let chain = e.api.get("/v1/chains/auth").await;
    assert_eq!(chain["summary"]["finish"]["state"], json!("released"));

    merge_card(&e, "Second", "b.txt").await;
    let chain = e.api.get("/v1/chains/auth").await;
    assert_eq!(
        chain["summary"]["finish"]["state"],
        json!("again"),
        "{chain}"
    );
    let offer = e.api.get("/v1/chains/auth/release").await;
    assert_eq!(offer["releasable"], json!(true));
    assert_eq!(offer["version"], json!("0.1.1"), "the next patch of v0.1.0");
    assert_eq!(offer["last_tag"], json!("v0.1.0"));

    e.api
        .post("/v1/chains/auth/release", json!({ "version": "0.1.1" }))
        .await;
    let run = finished_run(&e.api, "0.1.1").await;
    assert_eq!(run["state"], json!("done"), "{run}");
    assert_eq!(git(&e.repo, &["tag", "--list"]), "v0.1.0\nv0.1.1");
    let chain = e.api.get("/v1/chains/auth").await;
    assert_eq!(chain["summary"]["finish"]["state"], json!("released"));
}
