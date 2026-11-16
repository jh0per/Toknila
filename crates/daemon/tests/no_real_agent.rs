//! A daemon under a test root (`TOKNILA_TEST_ROOT`) refuses to launch a real agent program: the
//! start fails before any process, with the sentence the card blocks with.

use toknila_app::runner::{RunRequest, Runner, RunnerError};
use toknila_core::{AgentRole, TicketId};
use toknila_daemon::runner::{REAL_AGENT_REFUSAL, Runners};

fn request(dir: &std::path::Path, profile: &str) -> RunRequest {
    RunRequest {
        ticket: TicketId::new(),
        key: "DEMO-1".parse().expect("key"),
        role: AgentRole::Worker,
        profile: profile.to_owned(),
        dir: dir.display().to_string(),
        brief: Default::default(),
    }
}

#[test]
fn a_test_daemon_blocks_the_default_profile_and_spawns_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(dir.path().join("profiles")).expect("profiles");
    let (runners, mut ended) = Runners::new(dir.path().join("runs"), dir.path().join("profiles"));
    runners.set_refuse_real_agent(true);
    // Every built-in profile that names a program (all of them run `claude`).
    for name in ["claude-code", "claude-code-reviewer", "claude-code-planner"] {
        let err = runners
            .start(&request(dir.path(), name))
            .expect_err("a real agent must not start under a test root");
        assert!(
            err.to_string().contains(REAL_AGENT_REFUSAL),
            "{name}: {err}"
        );
    }
    assert_eq!(
        REAL_AGENT_REFUSAL,
        "A test may not start a real agent: give the project --sim or a fake --profile"
    );
    // Nothing was launched: no live run, no run record (log folder), no process end.
    assert!(runners.live_runners().is_empty());
    assert!(!dir.path().join("runs").exists(), "a run record was made");
    assert!(ended.try_recv().is_err());
}

#[test]
fn the_refusal_is_not_a_blanket_a_profile_file_passes_the_guard() {
    // Control: a profile a test wrote is not refused (here its program does not exist, so the start
    // fails later, with another reason than the refusal; nothing real is ever started).
    let dir = tempfile::tempdir().expect("tempdir");
    let profiles = dir.path().join("profiles");
    std::fs::create_dir_all(&profiles).expect("profiles");
    std::fs::write(
        profiles.join("fake.toml"),
        "command = ['/nonexistent/toknila-not-a-program']\n",
    )
    .expect("profile");
    let (runners, _ended) = Runners::new(dir.path().join("runs"), profiles);
    runners.set_refuse_real_agent(true);
    match runners.start(&request(dir.path(), "fake")) {
        Err(RunnerError::Spawn(m) | RunnerError::NoPty(m)) => {
            assert!(!m.contains(REAL_AGENT_REFUSAL), "{m}");
        }
        other => panic!("expected a spawn failure, got {other:?}"),
    }
}

// ---- The whole path: TOKNILA_TEST_ROOT -> Runners::new -> the card of a default-profile project.

use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::http::{Method, Request};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tokio::sync::Notify;
use toknila_daemon::server::{AppState, router};
use toknila_daemon::shipyard::GitShipyard;
use toknila_daemon::store::Store;
use tower::ServiceExt;

const CHILD: &str = "TOKNILA_NO_REAL_AGENT_CHILD";

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .output()
        .expect("git runs");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// The parent: runs the child test below in a copy of this binary with `TOKNILA_TEST_ROOT` set
/// (setting an environment variable in-process is `unsafe`, and other tests share the process),
/// so the switch itself, read in `Runners::new`, is what is proven. A deadline ends a hung child.
#[test]
fn a_daemon_under_test_root_blocks_the_card_of_a_default_profile_project() {
    let root = tempfile::tempdir().expect("tempdir");
    let mut child = Command::new(std::env::current_exe().expect("exe"))
        .args([
            "--exact",
            "the_child_with_the_test_root_set",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("TOKNILA_TEST_ROOT", root.path())
        .env(CHILD, "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn the child");
    let until = Instant::now() + Duration::from_secs(120);
    let status = loop {
        if let Some(s) = child.try_wait().expect("wait") {
            break s;
        }
        if Instant::now() >= until {
            let _ = child.kill();
            panic!("the child test did not end within its deadline");
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let mut out = String::new();
    let mut err = String::new();
    {
        use std::io::Read;
        child
            .stdout
            .take()
            .expect("out")
            .read_to_string(&mut out)
            .ok();
        child
            .stderr
            .take()
            .expect("err")
            .read_to_string(&mut err)
            .ok();
    }
    assert!(status.success(), "child failed:\n{out}\n{err}");
    // The child really ran (a filtered-out test also exits 0).
    assert!(out.contains("1 passed"), "child did not run:\n{out}");
}

/// The child: only runs when the parent set `CHILD`; the daemon here is built from the environment
/// alone (no `set_refuse_real_agent`).
#[tokio::test]
async fn the_child_with_the_test_root_set() {
    if std::env::var_os(CHILD).is_none() {
        return;
    }
    assert!(std::env::var_os("TOKNILA_TEST_ROOT").is_some());
    let tmp = tempfile::tempdir().expect("tmp");
    let repo = tmp.path().join("repo");
    std::fs::create_dir_all(&repo).expect("mkdir");
    git(&repo, &["init", "--quiet"]);
    git(&repo, &["symbolic-ref", "HEAD", "refs/heads/develop"]);
    std::fs::write(repo.join("README.md"), "hi\n").expect("write");
    git(&repo, &["add", "README.md"]);
    git(&repo, &["commit", "--quiet", "-m", "first"]);
    let profiles = tmp.path().join("p");
    std::fs::create_dir_all(&profiles).expect("profiles");
    let runs = tmp.path().join("runs");
    let (runners, _ended) = Runners::new(runs.clone(), profiles.clone());
    let store = Store::open(&tmp.path().join("t.db"))
        .expect("store")
        .with_shipyard(Arc::new(GitShipyard::new(tmp.path().join("ships"))))
        .with_merger(Arc::new(toknila_daemon::github::Mergers::new()))
        .with_runner(runners.clone());
    let app = router(AppState {
        store: store.clone(),
        token: "t".into(),
        shutdown: Arc::new(Notify::new()),
        profiles,
        runners: runners.clone(),
        tools: Arc::new(toknila_daemon::tools::SystemTools),
        usage: Arc::new(toknila_daemon::usage::Usage::disabled()),
        home: tmp.path().to_owned(),
        ships: tmp.path().join("ships"),
        trash: Arc::new(toknila_daemon::trash::NoTrash),
        pdf_renderer: Arc::new(toknila_daemon::designs::thumb::NoRenderer),
    });
    let call = |method: Method, uri: &'static str, body: Value| {
        let app = app.clone();
        let store = store.clone();
        async move {
            let req = Request::builder()
                .method(method)
                .uri(uri)
                .header("authorization", "Bearer t")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .expect("request");
            let resp = app.oneshot(req).await.expect("response");
            store.settled().await;
            let status = resp.status();
            let bytes = resp.into_body().collect().await.expect("body").to_bytes();
            let v: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
            assert!(status.is_success(), "{uri}: {status} {v}");
            v
        }
    };
    // No default_profile: the project gets the built-in default (the real agent).
    call(
        Method::POST,
        "/v1/projects",
        json!({ "key": "DEMO", "repo_path": repo.to_string_lossy() }),
    )
    .await;
    call(
        Method::POST,
        "/v1/projects/DEMO/chains",
        json!({ "name": "auth" }),
    )
    .await;
    call(
        Method::POST,
        "/v1/chains/auth/tickets",
        json!({ "title": "Add login", "prompt": "Let people in." }),
    )
    .await;
    let card = call(Method::GET, "/v1/tickets/DEMO-1", Value::Null).await;
    if card["ticket"]["status"] != "blocked" {
        call(Method::POST, "/v1/tickets/DEMO-1/actions/start", json!({})).await;
    }
    let card = call(Method::GET, "/v1/tickets/DEMO-1", Value::Null).await;
    assert_eq!(card["ticket"]["status"], "blocked", "{card}");
    let reason = card["ticket"]["status_reason"]["reason"].to_string();
    assert!(
        reason.contains(REAL_AGENT_REFUSAL),
        "the card does not show the sentence: {reason}"
    );
    assert!(runners.live_runners().is_empty());
    assert!(!runs.exists(), "a run record was made");
}
