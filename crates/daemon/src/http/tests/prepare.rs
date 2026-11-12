//! `POST /v1/repos/prepare`: making a missing or plain folder ready, with a real `git` in
//! temporary folders. The tools add a git identity (or take it away) so the machine's own
//! configuration does not matter.

use std::sync::Arc;
use std::time::Duration;

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};
use toknila_core::system::Probe;

use super::Api;
use crate::git::testing::git;
use crate::tools::{SystemTools, Tools};

/// Real git with these `-c` settings in front of every call.
#[derive(Debug)]
struct WithConfig(&'static [&'static str]);

impl Tools for WithConfig {
    fn run(&self, program: &str, args: &[&str], timeout: Duration) -> Probe {
        let mut all: Vec<&str> = Vec::new();
        if program == "git" {
            for kv in self.0 {
                all.extend(["-c", kv]);
            }
        }
        all.extend_from_slice(args);
        SystemTools.run(program, &all, timeout)
    }
}

const IDENTITY: WithConfig = WithConfig(&[
    "user.name=t",
    "user.email=t@example.com",
    "commit.gpgsign=false",
    "init.defaultBranch=trunk",
]);
const NOBODY: WithConfig = WithConfig(&["user.name=", "user.email="]);

async fn prepare(api: &Api, path: &str, fix: &str) -> (StatusCode, Value) {
    api.call(
        Method::POST,
        "/v1/repos/prepare",
        Some(json!({ "repo_path": path, "fix": fix })),
    )
    .await
}

#[tokio::test]
async fn create_folder_makes_the_folder_and_its_parents_and_offers_git_init_next() {
    let tmp = tempfile::tempdir().expect("tmp");
    let dir = tmp.path().join("a").join("b");
    let path = dir.to_string_lossy().into_owned();
    let api = Api::with_tools(Arc::new(IDENTITY));
    let (s, check) = prepare(&api, &path, "create_folder").await;
    assert_eq!(s, StatusCode::OK, "{check}");
    assert!(dir.is_dir());
    assert_eq!(check["problems"][0]["code"], "not_a_git_repo", "{check}");
    assert_eq!(check["problems"][0]["fix"], "init_repository");
}

#[tokio::test]
async fn init_repository_leaves_one_commit_and_a_check_without_problems() {
    let tmp = tempfile::tempdir().expect("tmp");
    let path = tmp.path().to_string_lossy().into_owned();
    let api = Api::with_tools(Arc::new(IDENTITY));
    let (s, check) = prepare(&api, &path, "init_repository").await;
    assert_eq!(s, StatusCode::OK, "{check}");
    assert_eq!(check["problems"], json!([]), "{check}");
    assert_eq!(check["suggested_branch"], "trunk");
    assert_eq!(check["branches"], json!(["trunk"]));
    assert_eq!(git(tmp.path(), &["rev-list", "--count", "HEAD"]), "1");
    assert_eq!(
        git(tmp.path(), &["log", "-1", "--format=%s"]),
        "Initial commit"
    );
}

#[tokio::test]
async fn first_commit_fixes_a_repository_without_a_commit() {
    let tmp = tempfile::tempdir().expect("tmp");
    git(tmp.path(), &["init", "--quiet"]);
    let path = tmp.path().to_string_lossy().into_owned();
    let api = Api::with_tools(Arc::new(IDENTITY));
    let c = api
        .post("/v1/repos/check", json!({ "repo_path": path }))
        .await;
    assert_eq!(c["problems"][0]["code"], "no_commits", "{c}");
    let (s, check) = prepare(&api, &path, "first_commit").await;
    assert_eq!(s, StatusCode::OK, "{check}");
    assert_eq!(check["problems"], json!([]), "{check}");
    assert_eq!(git(tmp.path(), &["rev-list", "--count", "HEAD"]), "1");
}

#[tokio::test]
async fn a_fix_the_check_does_not_offer_is_refused_and_nothing_is_touched() {
    let tmp = tempfile::tempdir().expect("tmp");
    let path = tmp.path().to_string_lossy().into_owned();
    let api = Api::with_tools(Arc::new(IDENTITY));
    let (s, e) = prepare(&api, &path, "first_commit").await;
    assert_eq!(s, StatusCode::CONFLICT, "{e}");
    assert_eq!(e["code"], "conflict");
    assert!(!tmp.path().join(".git").exists(), "folder unchanged");
    let gone = tmp.path().join("gone").to_string_lossy().into_owned();
    let (s, _) = prepare(&api, &gone, "init_repository").await;
    assert_eq!(s, StatusCode::CONFLICT);
    assert!(!tmp.path().join("gone").exists());
}

#[tokio::test]
async fn without_a_git_identity_the_commit_is_refused_naming_the_two_commands() {
    let tmp = tempfile::tempdir().expect("tmp");
    let path = tmp.path().to_string_lossy().into_owned();
    let api = Api::with_tools(Arc::new(NOBODY));
    let (s, e) = prepare(&api, &path, "init_repository").await;
    assert_eq!(s, StatusCode::CONFLICT, "{e}");
    let text = e["error"].as_str().expect("error text");
    assert!(text.contains("git config --global user.name"), "{text}");
    assert!(text.contains("user.email"), "{text}");
    assert!(!tmp.path().join(".git").exists(), "folder unchanged");

    // A repository without a commit stays one, and the check goes on offering the commit.
    git(tmp.path(), &["init", "--quiet"]);
    let (s, e) = prepare(&api, &path, "first_commit").await;
    assert_eq!(s, StatusCode::CONFLICT, "{e}");
    let c = api
        .post("/v1/repos/check", json!({ "repo_path": path }))
        .await;
    assert_eq!(c["problems"][0]["fix"], "first_commit", "{c}");
}

#[tokio::test]
async fn an_agent_and_a_planner_token_are_refused() {
    let api = Api::new();
    api.demo_with(&["A"]).await;
    let planner = api.planner_token("DEMO").await;
    let (agent, _) = api.agent_token("DEMO-1").await;
    for token in [planner, agent] {
        let (s, e) = api
            .call_with(
                &token,
                Method::POST,
                "/v1/repos/prepare",
                Some(json!({ "repo_path": "/nowhere", "fix": "create_folder" })),
            )
            .await;
        assert_eq!(s, StatusCode::FORBIDDEN, "{e}");
    }
}
