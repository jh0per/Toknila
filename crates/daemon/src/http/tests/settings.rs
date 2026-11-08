//! Settings and onboarding through the router: the system checks (with injected tools, so the
//! machine running the tests needs neither `claude` nor `gh`), the folder checks of a new
//! project, the project settings the window edits, and that they survive a restart.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};
use toknila_core::system::Probe;

use super::Api;
use crate::git::testing::{git, init_repo};
use crate::tools::Tools;

/// Answers by `program args...`; a program it does not know is missing.
#[derive(Debug, Default)]
struct FakeTools(HashMap<String, Probe>);

impl FakeTools {
    fn with(mut self, call: &str, probe: Probe) -> Self {
        self.0.insert(call.to_owned(), probe);
        self
    }
}

impl Tools for FakeTools {
    fn run(&self, program: &str, args: &[&str], _timeout: Duration) -> Probe {
        let call = std::iter::once(program)
            .chain(args.iter().copied())
            .collect::<Vec<_>>();
        self.0
            .get(&call.join(" "))
            .cloned()
            .unwrap_or(Probe::Missing)
    }
}

fn ran(code: i32, stdout: &str) -> Probe {
    Probe::Ran {
        code: Some(code),
        stdout: stdout.to_owned(),
        stderr: String::new(),
    }
}

fn check<'a>(body: &'a Value, tool: &str) -> &'a Value {
    body["checks"]
        .as_array()
        .and_then(|all| all.iter().find(|c| c["tool"] == tool))
        .unwrap_or_else(|| panic!("no {tool} check in {body}"))
}

#[tokio::test]
async fn system_checks_report_each_tool_with_a_hint() {
    let tools = FakeTools::default()
        .with(
            "git --version",
            ran(0, "git version 2.39.5 (Apple Git-154)\n"),
        )
        .with("gh --version", ran(0, "gh version 2.40.0 (2023-12-12)\n"))
        .with("gh auth status", ran(1, ""));
    let api = Api::with_tools(Arc::new(tools));
    let body = api.get("/v1/system/checks").await;
    assert_eq!(body["ok"], false);
    let git = check(&body, "git");
    assert_eq!(
        (git["status"].as_str(), git["version"].as_str()),
        (Some("ok"), Some("2.39.5"))
    );
    let claude = check(&body, "claude");
    assert_eq!(claude["status"], "missing");
    assert!(
        claude["hint"]
            .as_str()
            .is_some_and(|h| h.contains("claude")),
        "{claude}"
    );
    let gh = check(&body, "gh");
    assert_eq!(gh["status"], "not_logged_in");
    assert_eq!(gh["hint"], "Run `gh auth login` and follow the prompts.");
}

/// The version is the build's and nothing is linked in a test run (whether a `tok` sits beside the
/// test binary depends on the platform's build layout, so that is not asserted here).
#[tokio::test]
async fn cli_status_names_the_version_and_links_nothing() {
    let body = Api::new().get("/v1/system/cli").await;
    assert_eq!(body["version"], toknila_core::VERSION);
    assert_eq!(body["installed"], false);
}

#[tokio::test]
async fn system_checks_are_ok_when_every_tool_is() {
    let tools = FakeTools::default()
        .with("git --version", ran(0, "git version 2.43.0\n"))
        .with("claude --version", ran(0, "2.1.319 (Claude Code)\n"))
        .with("gh --version", ran(0, "gh version 2.40.0 (2023-12-12)\n"))
        .with("gh auth status", ran(0, ""));
    let api = Api::with_tools(Arc::new(tools));
    assert_eq!(api.get("/v1/system/checks").await["ok"], true);
}

/// A repository whose only branch is `develop`, with one commit.
fn repo(dir: &Path) -> PathBuf {
    let repo = dir.join("repo");
    init_repo(&repo, "develop");
    repo
}

async fn refused(api: &Api, body: Value) -> Value {
    let (s, e) = api.call(Method::POST, "/v1/projects", Some(body)).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{e}");
    e
}

#[tokio::test]
async fn a_new_project_with_a_bad_folder_is_refused_with_a_typed_error_per_field() {
    let tmp = tempfile::tempdir().expect("tmp");
    let good = repo(tmp.path());
    let plain = tmp.path().join("plain");
    std::fs::create_dir_all(&plain).expect("mkdir");
    let file = good.join("README.md");
    let api = Api::new();
    let path = |p: &Path| p.to_string_lossy().into_owned();

    for (repo_path, code) in [
        (path(&tmp.path().join("nope")), "path_missing"),
        (path(&file), "not_a_directory"),
        (path(&plain), "not_a_git_repo"),
        ("relative/dir".to_owned(), "path_not_absolute"),
    ] {
        let e = refused(&api, json!({ "key": "DEMO", "repo_path": repo_path })).await;
        assert_eq!(
            (e["code"].as_str(), e["field"].as_str()),
            (Some(code), Some("repo_path"))
        );
        assert!(e["error"].as_str().is_some_and(|m| !m.is_empty()), "{e}");
    }
    let e = refused(
        &api,
        json!({ "key": "DEMO", "repo_path": path(&good), "work_branch": "main" }),
    )
    .await;
    assert_eq!(
        (e["code"].as_str(), e["field"].as_str()),
        (Some("branch_missing"), Some("work_branch"))
    );
    // Nothing was created by any refusal.
    assert_eq!(api.get("/v1/projects").await, json!([]));

    let ok = api
        .post(
            "/v1/projects",
            json!({ "key": "DEMO", "repo_path": path(&good) }),
        )
        .await;
    assert_eq!(ok["work_branch"], "develop");
}

#[tokio::test]
async fn the_folder_check_lists_branches_to_pick_when_the_base_is_missing() {
    let tmp = tempfile::tempdir().expect("tmp");
    let good = repo(tmp.path());
    git(&good, &["branch", "trunk"]);
    let api = Api::new();
    let path = good.to_string_lossy().into_owned();

    let ok = api
        .post(
            "/v1/repos/check",
            json!({ "repo_path": path, "work_branch": "develop" }),
        )
        .await;
    assert_eq!(ok["problems"], json!([]));
    let miss = api
        .post(
            "/v1/repos/check",
            json!({ "repo_path": path, "work_branch": "main" }),
        )
        .await;
    assert_eq!(miss["problems"][0]["code"], "branch_missing");
    assert_eq!(miss["problems"][0]["field"], "work_branch");
    assert_eq!(miss["branches"], json!(["develop", "trunk"]));
    // The base defaults to develop, as a new project's does.
    let default = api
        .post("/v1/repos/check", json!({ "repo_path": path }))
        .await;
    assert_eq!(default["problems"], json!([]));
}

#[tokio::test]
async fn changing_the_base_branch_checks_it_against_the_repository() {
    let tmp = tempfile::tempdir().expect("tmp");
    let good = repo(tmp.path());
    git(&good, &["branch", "trunk"]);
    let api = Api::new();
    api.post(
        "/v1/projects",
        json!({ "key": "DEMO", "repo_path": good.to_string_lossy() }),
    )
    .await;

    let (s, e) = api
        .call(
            Method::PATCH,
            "/v1/projects/DEMO",
            Some(json!({ "work_branch": "nope" })),
        )
        .await;
    assert_eq!(
        (s, e["code"].as_str(), e["field"].as_str()),
        (
            StatusCode::BAD_REQUEST,
            Some("branch_missing"),
            Some("work_branch")
        )
    );
    assert_eq!(api.get("/v1/projects/DEMO").await["work_branch"], "develop");

    let (s, p) = api
        .call(
            Method::PATCH,
            "/v1/projects/DEMO",
            Some(json!({ "work_branch": "trunk" })),
        )
        .await;
    assert_eq!(
        (s, p["work_branch"].as_str()),
        (StatusCode::OK, Some("trunk"))
    );
}

#[tokio::test]
async fn the_default_profile_must_be_in_the_catalog() {
    let api = Api::new();
    api.post("/v1/projects", json!({ "key": "DEMO" })).await;
    let (s, e) = api
        .call(
            Method::PATCH,
            "/v1/projects/DEMO",
            Some(json!({ "default_profile": "ghost" })),
        )
        .await;
    assert_eq!(
        (s, e["field"].as_str()),
        (StatusCode::BAD_REQUEST, Some("default_profile"))
    );
    let (s, p) = api
        .call(
            Method::PATCH,
            "/v1/projects/DEMO",
            Some(json!({ "default_profile": "claude-code" })),
        )
        .await;
    assert_eq!(
        (s, p["default_profile"].as_str()),
        (StatusCode::OK, Some("claude-code"))
    );
}

#[tokio::test]
async fn an_out_of_range_limit_is_refused_at_its_field() {
    let api = Api::new();
    api.post("/v1/projects", json!({ "key": "DEMO" })).await;
    let (s, e) = api
        .call(
            Method::PATCH,
            "/v1/projects/DEMO",
            Some(json!({ "max_parallel": 99 })),
        )
        .await;
    assert_eq!(
        (s, e["field"].as_str()),
        (StatusCode::BAD_REQUEST, Some("max_parallel"))
    );
    assert!(
        e["error"]
            .as_str()
            .is_some_and(|m| m.contains("between 1 and 64")),
        "{e}"
    );
    assert_eq!(api.get("/v1/projects/DEMO").await["max_parallel"], 2);
}

#[tokio::test]
async fn project_settings_survive_a_daemon_restart() {
    let api = Api::new();
    api.post("/v1/projects", json!({ "key": "DEMO" })).await;
    let (s, _) = api
        .call(
            Method::PATCH,
            "/v1/projects/DEMO",
            Some(json!({
                "max_parallel": 5,
                "review_mode": "both",
                "reviewer": "sim-reviewer",
                "default_profile": "claude-code",
            })),
        )
        .await;
    assert_eq!(s, StatusCode::OK);

    let api = api.restart();
    let p = api.get("/v1/projects/DEMO").await;
    assert_eq!(p["max_parallel"], 5);
    assert_eq!(p["default_review_mode"], "both");
    assert_eq!(p["default_reviewer"], "sim-reviewer");
    assert_eq!(p["default_profile"], "claude-code");
}

#[tokio::test]
async fn the_check_command_is_saved_cleared_refused_at_its_field_and_survives_a_restart() {
    let api = Api::new();
    api.post("/v1/projects", json!({ "key": "DEMO" })).await;
    assert!(
        api.get("/v1/projects/DEMO")
            .await
            .get("check_command")
            .is_none()
    );
    let patch = |body: Value| api.call(Method::PATCH, "/v1/projects/DEMO", Some(body));

    let (s, p) = patch(json!({ "check_command": " cargo xtask verify " })).await;
    assert_eq!(
        (s, p["check_command"].as_str()),
        (StatusCode::OK, Some("cargo xtask verify"))
    );

    let (s, e) = patch(json!({ "check_command": "make (all)" })).await;
    assert_eq!(
        (s, e["field"].as_str()),
        (StatusCode::BAD_REQUEST, Some("check_command"))
    );
    let still = api.get("/v1/projects/DEMO").await;
    assert_eq!(
        still["check_command"], "cargo xtask verify",
        "a refused value changes nothing"
    );

    let api = api.restart();
    assert_eq!(
        api.get("/v1/projects/DEMO").await["check_command"],
        "cargo xtask verify"
    );
    let (s, p) = api
        .call(
            Method::PATCH,
            "/v1/projects/DEMO",
            Some(json!({ "check_command": "" })),
        )
        .await;
    assert_eq!(s, StatusCode::OK);
    assert!(
        p.get("check_command").is_none(),
        "an empty text clears it: {p}"
    );
}

#[tokio::test]
async fn a_project_without_a_base_takes_the_branch_the_repository_is_on() {
    let tmp = tempfile::tempdir().expect("tmp");
    let main = repo(tmp.path());
    git(&main, &["branch", "-m", "develop", "main"]);
    git(&main, &["branch", "develop"]);
    let path = main.to_string_lossy().into_owned();
    let api = Api::new();

    // The folder check suggests it, and judges the base against it.
    let check = api
        .post("/v1/repos/check", json!({ "repo_path": path }))
        .await;
    assert_eq!(check["suggested_branch"], "main");
    assert_eq!(check["problems"], json!([]));

    let made = api
        .post("/v1/projects", json!({ "key": "DEMO", "repo_path": path }))
        .await;
    assert_eq!(made["work_branch"], "main");
    // A base that was named stays as it was named.
    let named = api
        .post(
            "/v1/projects",
            json!({ "key": "NAMED", "repo_path": path, "work_branch": "develop" }),
        )
        .await;
    assert_eq!(named["work_branch"], "develop");
}

#[tokio::test]
async fn a_detached_head_falls_back_to_the_remote_default_and_then_names_the_choices() {
    let tmp = tempfile::tempdir().expect("tmp");
    let origin = repo(tmp.path());
    let clone = tmp.path().join("clone");
    git(
        tmp.path(),
        &["clone", "--quiet", &origin.to_string_lossy(), "clone"],
    );
    git(&clone, &["branch", "other"]);
    git(&clone, &["checkout", "--quiet", "--detach"]);
    let path = clone.to_string_lossy().into_owned();
    let api = Api::new();
    let made = api
        .post("/v1/projects", json!({ "key": "DEMO", "repo_path": path }))
        .await;
    assert_eq!(made["work_branch"], "develop", "origin's default");

    // No remote either: an error naming the branches to choose from.
    git(&clone, &["remote", "remove", "origin"]);
    let e = refused(&api, json!({ "key": "TWO", "repo_path": path })).await;
    assert_eq!(
        (e["code"].as_str(), e["field"].as_str()),
        (Some("branch_missing"), Some("work_branch"))
    );
    assert_eq!(
        e["error"],
        "cannot tell the base branch: HEAD is not on a branch and the remote names no default; \
         pass one of the repository's branches (develop, other)"
    );
    assert_eq!(
        api.get("/v1/projects").await.as_array().map(Vec::len),
        Some(1)
    );
}
