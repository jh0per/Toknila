//! Plans found in a project's repository, and importing them by path, through the router.

use std::path::{Path, PathBuf};

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};

use super::Api;
use super::ships::repo;

const TASKS: &str = "## Phase 1: Setup\n- [x] T001 a\n\n## Phase 2: Build\n- [ ] T002 b\n";
const CHAIN: &str = "[chain]\nname = \"phase-1\"\n\n[[ticket]]\nid = \"A\"\ntitle = \"A\"\n";

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    std::fs::write(path, text).expect("write");
}

/// Project DEMO on a repository holding two spec folders and one chain file.
async fn planned(tmp: &Path) -> (Api, PathBuf) {
    let repo = repo(tmp);
    write(&repo.join("specs/001-login/tasks.md"), TASKS);
    write(&repo.join("specs/002-search/tasks.md"), TASKS);
    write(&repo.join("plans/phase-1.toml"), CHAIN);
    let api = Api::new();
    api.post(
        "/v1/projects",
        json!({ "key": "DEMO", "repo_path": repo.to_string_lossy() }),
    )
    .await;
    (api, repo)
}

/// `relative imported` of each plan the scan lists.
async fn scan(api: &Api) -> Vec<(String, bool)> {
    let s = api.get("/v1/projects/DEMO/plans").await;
    s["plans"]
        .as_array()
        .expect("plans")
        .iter()
        .map(|p| {
            (
                p["relative"].as_str().expect("relative").to_owned(),
                p["imported"].as_bool().expect("imported"),
            )
        })
        .collect()
}

async fn import(api: &Api, path: &str) -> (StatusCode, Value) {
    api.call(
        Method::POST,
        "/v1/projects/DEMO/import-path",
        Some(json!({ "path": path })),
    )
    .await
}

fn path_of(scan: &Value, relative: &str) -> String {
    scan["plans"]
        .as_array()
        .expect("plans")
        .iter()
        .find(|p| p["relative"] == relative)
        .and_then(|p| p["path"].as_str())
        .expect("listed")
        .to_owned()
}

#[tokio::test]
async fn the_scan_finds_the_plans_and_marks_the_ones_imported_by_path() {
    let tmp = tempfile::tempdir().expect("tmp");
    let (api, _repo) = planned(tmp.path()).await;
    let first = api.get("/v1/projects/DEMO/plans").await;
    assert_eq!(first["specify_dir"], false);
    assert_eq!(
        scan(&api).await,
        [
            ("plans/phase-1.toml".to_owned(), false),
            ("specs/001-login".to_owned(), false),
            ("specs/002-search".to_owned(), false),
        ]
    );
    assert_eq!(first["plans"][1]["kind"], "spec_folder");
    assert_eq!(first["plans"][1]["cards"], 2);
    assert_eq!(first["plans"][0]["kind"], "chain_file");

    for relative in ["specs/001-login", "plans/phase-1.toml"] {
        let path = path_of(&first, relative);
        let (s, out) = import(&api, &path).await;
        assert_eq!(s, StatusCode::CREATED, "{out}");
        assert_eq!(out["chains"][0]["path"], path.as_str());
        assert_eq!(out["chains"][0]["chain"]["chain"]["source"], path.as_str());
    }
    assert_eq!(
        scan(&api).await,
        [
            ("plans/phase-1.toml".to_owned(), true),
            ("specs/001-login".to_owned(), true),
            ("specs/002-search".to_owned(), false),
        ]
    );
    let chain = api.get("/v1/chains/001-login").await;
    assert_eq!(chain["tickets"].as_array().map(Vec::len), Some(2));
}

#[tokio::test]
async fn import_by_path_refuses_clearly_and_names_the_path() {
    let tmp = tempfile::tempdir().expect("tmp");
    let (api, repo) = planned(tmp.path()).await;
    let (s, e) = import(&api, "specs/001-login").await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{e}");
    assert_eq!(e["error"], "specs/001-login is not an absolute path");

    let gone = repo.join("specs").join("404");
    let (s, e) = import(&api, &gone.to_string_lossy()).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{e}");
    assert_eq!(e["error"], format!("{} does not exist", gone.display()));

    let broken = repo.join("specs/003-broken/tasks.md");
    write(&broken, "## Phase 1: A\n- [ ] T1 ok\n- [q] T2 bad\n");
    let (s, e) = import(&api, &broken.parent().expect("folder").to_string_lossy()).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{e}");
    let message = e["error"].as_str().expect("message");
    assert!(
        message.ends_with("tasks.md:3: malformed task line: expected `- [ ] text` or `- [x] text`"),
        "{message}"
    );
    assert!(message.contains("003-broken"), "{message}");

    let path = path_of(&api.get("/v1/projects/DEMO/plans").await, "specs/001-login");
    import(&api, &path).await;
    let (s, e) = import(&api, &path).await;
    assert_eq!(s, StatusCode::CONFLICT, "{e}");
    assert_eq!(
        e["error"],
        format!("{path}: epic 001-login already exists in DEMO")
    );
}

#[tokio::test]
async fn a_folder_of_spec_folders_imports_each_and_reports_the_rest() {
    let tmp = tempfile::tempdir().expect("tmp");
    let (api, repo) = planned(tmp.path()).await;
    write(&repo.join("specs/003-broken/tasks.md"), "- [q] T1 bad\n");
    std::fs::create_dir_all(repo.join("specs/004-notes")).expect("mkdir");
    let (s, out) = import(&api, &repo.join("specs").to_string_lossy()).await;
    assert_eq!(s, StatusCode::CREATED, "{out}");
    let names = |list: &Value| -> Vec<String> {
        list.as_array()
            .expect("list")
            .iter()
            .map(|x| x["folder"].as_str().expect("folder").to_owned())
            .collect()
    };
    assert_eq!(names(&out["chains"]), ["001-login", "002-search"]);
    assert_eq!(names(&out["failed"]), ["003-broken"]);
    assert_eq!(names(&out["skipped"]), ["004-notes"]);
    assert_eq!(out["skipped"][0]["reason"], "no tasks.md");
    assert_eq!(out["chains"][0]["cards"], 2);
    assert_eq!(out["chains"][0]["done_cards"], 1);

    // Again: nothing new, so 200, and each spec folder says why.
    let (s, out) = import(&api, &repo.join("specs").to_string_lossy()).await;
    assert_eq!(s, StatusCode::OK, "{out}");
    assert_eq!(
        names(&out["failed"]),
        ["001-login", "002-search", "003-broken"]
    );
}

#[tokio::test]
async fn a_project_without_a_repository_has_no_plans() {
    let api = Api::new();
    api.post("/v1/projects", json!({ "key": "DEMO" })).await;
    let s = api.get("/v1/projects/DEMO/plans").await;
    assert_eq!(s, json!({ "plans": [], "specify_dir": false }));
    let (status, _) = api.call(Method::GET, "/v1/projects/NOPE/plans", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
