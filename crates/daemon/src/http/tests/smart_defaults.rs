//! What a new project is offered from its repository (`POST /v1/repos/check`) and the project
//! setting for the agents' Claude model, through the router. Real folders and a real `git`; only
//! `gh` is answered by the test, so the machine's own login does not matter.

use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};
use toknila_core::system::Probe;

use super::Api;
use crate::tools::{SystemTools, Tools};

/// Real tools, except that `gh auth status` answers `code`.
#[derive(Debug)]
struct GhAnswers(i32);

impl Tools for GhAnswers {
    fn run(&self, program: &str, args: &[&str], timeout: Duration) -> Probe {
        if program == "gh" {
            return Probe::Ran {
                code: Some(self.0),
                stdout: String::new(),
                stderr: String::new(),
            };
        }
        SystemTools.run(program, args, timeout)
    }
}

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .output()
        .expect("git runs");
    assert!(out.status.success(), "git {args:?}: {out:?}");
}

/// A repository on `main` with these files (relative path, text) and, if given, an `origin`.
fn repo(dir: &Path, name: &str, files: &[(&str, &str)], origin: Option<&str>) -> String {
    let repo = dir.join(name);
    std::fs::create_dir_all(&repo).expect("mkdir");
    git(&repo, &["init", "--quiet"]);
    git(&repo, &["symbolic-ref", "HEAD", "refs/heads/main"]);
    for (path, text) in files.iter().chain(&[("a.txt", "a")]) {
        let file = repo.join(path);
        std::fs::create_dir_all(file.parent().expect("parent")).expect("mkdir");
        std::fs::write(file, text).expect("write");
    }
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "--quiet", "-m", "init"]);
    if let Some(url) = origin {
        git(&repo, &["remote", "add", "origin", url]);
    }
    repo.to_string_lossy().into_owned()
}

async fn suggestion(api: &Api, path: &str) -> (Value, Value) {
    let c = api
        .post("/v1/repos/check", json!({ "repo_path": path }))
        .await;
    assert_eq!(c["problems"], json!([]), "{c}");
    (
        c["suggested_check_command"].clone(),
        c["suggested_merge_mode"].clone(),
    )
}

const GITHUB: &str = "git@github.com:jh0per/demo.git";

#[tokio::test]
async fn the_folder_check_suggests_the_check_command_each_kind_of_repository_runs() {
    let tmp = tempfile::tempdir().expect("tmp");
    let api = Api::with_tools(Arc::new(GhAnswers(1)));
    let cases = [
        (
            "xtask",
            vec![
                ("Cargo.toml", "[workspace]"),
                ("xtask/src/main.rs", "match a { \"verify\" => v() }"),
            ],
            json!("cargo xtask verify"),
        ),
        (
            "xtask-without-verify",
            vec![
                ("Cargo.toml", "[workspace]"),
                ("xtask/src/main.rs", "match a { \"guards\" => g() }"),
            ],
            json!("cargo test"),
        ),
        (
            "cargo",
            vec![("Cargo.toml", "[package]")],
            json!("cargo test"),
        ),
        (
            "npm",
            vec![("package.json", r#"{"scripts":{"test":"vitest run"}}"#)],
            json!("npm test"),
        ),
        (
            "pnpm",
            vec![
                ("package.json", r#"{"scripts":{"test":"vitest run"}}"#),
                ("pnpm-lock.yaml", ""),
            ],
            json!("pnpm test"),
        ),
        (
            "yarn",
            vec![
                ("package.json", r#"{"scripts":{"test":"jest"}}"#),
                ("yarn.lock", ""),
            ],
            json!("yarn test"),
        ),
        (
            "make",
            vec![("Makefile", "CC=cc\n\ntest: all\n\t./run-tests\n")],
            json!("make test"),
        ),
        (
            "python",
            vec![("pyproject.toml", "[project]"), ("tests/test_a.py", "")],
            json!("pytest"),
        ),
        (
            "nothing",
            vec![
                ("package.json", r#"{"scripts":{"build":"tsc"}}"#),
                ("Makefile", "build:\n\tcc x.c\n"),
                ("pyproject.toml", "[project]"),
            ],
            json!(null),
        ),
    ];
    for (name, files, want) in cases {
        let path = repo(tmp.path(), name, &files, None);
        let (check, merge) = suggestion(&api, &path).await;
        assert_eq!(check, want, "{name}");
        assert_eq!(merge, "local", "{name}: no remote");
    }
}

#[tokio::test]
async fn pull_requests_are_suggested_for_a_github_remote_with_a_gh_login_only() {
    let tmp = tempfile::tempdir().expect("tmp");
    let hub = repo(tmp.path(), "hub", &[], Some(GITHUB));
    let other = repo(tmp.path(), "other", &[], Some("https://gitlab.com/o/r.git"));

    let logged_in = Api::with_tools(Arc::new(GhAnswers(0)));
    assert_eq!(suggestion(&logged_in, &hub).await.1, "github");
    assert_eq!(suggestion(&logged_in, &other).await.1, "local");
    let logged_out = Api::with_tools(Arc::new(GhAnswers(1)));
    assert_eq!(suggestion(&logged_out, &hub).await.1, "local");

    // A folder that is no repository suggests nothing.
    let none = logged_in
        .post(
            "/v1/repos/check",
            json!({ "repo_path": tmp.path().join("nope") }),
        )
        .await;
    assert_eq!(none["suggested_merge_mode"], json!(null));
    assert_eq!(none["suggested_check_command"], json!(null));
}

#[tokio::test]
async fn a_project_is_created_with_the_suggested_check_command_and_merge_mode() {
    let tmp = tempfile::tempdir().expect("tmp");
    let path = repo(
        tmp.path(),
        "hub",
        &[("Cargo.toml", "[package]")],
        Some(GITHUB),
    );
    let api = Api::new();
    let made = api
        .post(
            "/v1/projects",
            json!({ "key": "HUB", "repo_path": path, "check_command": "cargo test", "merge_mode": "github" }),
        )
        .await;
    assert_eq!(made["check_command"], "cargo test");
    assert_eq!(made["merge"]["mode"], "github");
    let api = api.restart();
    let again = api.get("/v1/projects/HUB").await;
    assert_eq!(again["check_command"], "cargo test");
    assert_eq!(again["merge"]["mode"], "github");

    // A pull-request project needs a repository; a bad check command is the field's problem.
    let (s, _) = api
        .call(
            Method::POST,
            "/v1/projects",
            Some(json!({ "key": "NOREPO", "merge_mode": "github" })),
        )
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let (s, e) = api
        .call(
            Method::POST,
            "/v1/projects",
            Some(json!({ "key": "BAD", "check_command": "make (all)" })),
        )
        .await;
    assert_eq!(
        (s, e["field"].as_str()),
        (StatusCode::BAD_REQUEST, Some("check_command")),
        "{e}"
    );
}

#[tokio::test]
async fn the_places_name_the_data_home_the_ships_and_the_profiles_folder() {
    let api = Api::new();
    let p = api.get("/v1/system/places").await;
    let (home, ships, profiles) = (
        p["home"].as_str().expect("home"),
        p["ships"].as_str().expect("ships"),
        p["profiles"].as_str().expect("profiles"),
    );
    assert_eq!(ships, format!("{home}{}ships", std::path::MAIN_SEPARATOR));
    assert_eq!(
        profiles,
        format!("{home}{}profiles", std::path::MAIN_SEPARATOR)
    );
}

#[tokio::test]
async fn the_models_per_role_are_saved_cleared_refused_at_their_field_and_survive_a_restart() {
    let api = Api::new();
    api.post("/v1/projects", json!({ "key": "DEMO" })).await;
    let before = api.get("/v1/projects/DEMO").await;
    for f in ["worker_model", "reviewer_model", "planner_model"] {
        assert!(before.get(f).is_none(), "{f}: {before}");
    }
    let patch = |body: Value| api.call(Method::PATCH, "/v1/projects/DEMO", Some(body));

    let (s, p) = patch(json!({ "worker_model": " sonnet ", "planner_model": "opus" })).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(
        (
            p["worker_model"].as_str(),
            p["reviewer_model"].as_str(),
            p["planner_model"].as_str()
        ),
        (Some("sonnet"), None, Some("opus")),
        "each role keeps its own: {p}"
    );
    for field in ["worker_model", "reviewer_model", "planner_model"] {
        let (s, e) = patch(json!({ field: "claude opus" })).await;
        assert_eq!(
            (s, e["field"].as_str()),
            (StatusCode::BAD_REQUEST, Some(field)),
            "{e}"
        );
    }
    let still = api.get("/v1/projects/DEMO").await;
    assert_eq!(
        (
            still["worker_model"].as_str(),
            still["planner_model"].as_str()
        ),
        (Some("sonnet"), Some("opus")),
        "a refused patch changes nothing"
    );

    let api = api.restart();
    let again = api.get("/v1/projects/DEMO").await;
    assert_eq!(
        (
            again["worker_model"].as_str(),
            again["planner_model"].as_str()
        ),
        (Some("sonnet"), Some("opus"))
    );
    let (s, p) = api
        .call(
            Method::PATCH,
            "/v1/projects/DEMO",
            Some(json!({ "worker_model": "", "reviewer_model": "haiku" })),
        )
        .await;
    assert_eq!(s, StatusCode::OK);
    assert!(
        p.get("worker_model").is_none(),
        "an empty text clears it: {p}"
    );
    assert_eq!(p["reviewer_model"], "haiku");
    assert_eq!(p["planner_model"], "opus", "the others stay");
}
