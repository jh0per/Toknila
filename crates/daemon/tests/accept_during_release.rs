//! A release and the merges take turns, through the full router with real git ships, the real
//! shell checker and the real release run. The check command adds one entry to the repository's
//! git config (`toknila.checks`, shared by every worktree) and then sleeps in the workspace's
//! `fake-agent`, so a check is counted and slow; there is no shell redirect, so the same words
//! work in `sh` and in `cmd.exe`. The bump command writes the version with `git config --file`.
//! Every wait has a deadline.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tokio::sync::Notify;
use toknila_app::checks::{CheckGate, CheckKind};
use toknila_daemon::server::{AppState, router};
use toknila_daemon::shipyard::GitShipyard;
use toknila_daemon::store::Store;
use tower::ServiceExt;

const TOKEN: &str = "t";
/// How long the check command sleeps: long enough for a person's Accept to land meanwhile.
const SLOW_MS: u64 = 1500;
const DEADLINE: Duration = Duration::from_secs(60);

fn git(dir: &Path, args: &[&str]) -> (bool, String) {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .output()
        .expect("git runs");
    let text = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    (out.status.success(), text)
}

fn git_ok(dir: &Path, args: &[&str]) -> String {
    let (ok, out) = git(dir, args);
    assert!(ok, "git {args:?} failed");
    out
}

fn commit_file(dir: &Path, name: &str) {
    std::fs::write(dir.join(name), format!("{name}\n")).expect("write");
    git_ok(dir, &["add", name]);
    git_ok(dir, &["commit", "--quiet", "-m", name]);
}

/// How many checks ran (the command counts itself in the repository's config).
fn checks(repo: &Path) -> usize {
    let (_, out) = git(repo, &["config", "--get-all", "toknila.checks"]);
    out.lines().count()
}

fn check_command() -> String {
    let agent = env!("CARGO_BIN_EXE_fake-agent");
    let exe = if agent.contains(' ') {
        format!("\"{agent}\"")
    } else {
        agent.to_owned()
    };
    format!("git config --add toknila.checks 1 && {exe} sleep {SLOW_MS}")
}

struct Rig {
    _tmp: tempfile::TempDir,
    app: axum::Router,
    store: Store,
    repo: PathBuf,
}

impl Rig {
    /// Project DEMO (simulated agents, a person reviews) with the slow check and a bump; epic
    /// `base` holds DEMO-1, merged, so it can be released; epic `auth` holds DEMO-2, started.
    async fn new() -> Rig {
        let tmp = tempfile::tempdir().expect("tmp");
        let repo = tmp.path().join("repo");
        std::fs::create_dir_all(&repo).expect("mkdir");
        git_ok(&repo, &["init", "--quiet"]);
        git_ok(&repo, &["symbolic-ref", "HEAD", "refs/heads/develop"]);
        commit_file(&repo, "README.md");
        let (runners, _) =
            toknila_daemon::runner::Runners::new(tmp.path().join("runs"), tmp.path().join("p"));
        let store = Store::open(&tmp.path().join("t.db"))
            .expect("store")
            .with_shipyard(Arc::new(GitShipyard::new(tmp.path().join("ships"))))
            .with_merger(Arc::new(toknila_daemon::merger::GitMerger::new()))
            .with_runner(runners.clone());
        let app = router(AppState {
            store: store.clone(),
            token: TOKEN.into(),
            shutdown: Arc::new(Notify::new()),
            profiles: tmp.path().join("p"),
            runners,
            tools: Arc::new(toknila_daemon::tools::SystemTools),
            usage: Arc::new(toknila_daemon::usage::Usage::disabled()),
            home: tmp.path().to_owned(),
            ships: tmp.path().join("ships"),
            trash: Arc::new(toknila_daemon::trash::NoTrash),
            pdf_renderer: Arc::new(toknila_daemon::designs::thumb::NoRenderer),
        });
        let rig = Rig {
            _tmp: tmp,
            app,
            store,
            repo: repo.clone(),
        };
        rig.call(
            Method::POST,
            "/v1/projects",
            json!({ "key": "DEMO", "repo_path": repo.to_string_lossy(), "default_profile": "sim",
                    "simulated": true, "default_review_mode": "human" }),
        )
        .await;
        rig.call(
            Method::PATCH,
            "/v1/projects/DEMO",
            json!({ "check_command": check_command(),
                    "bump_command": "git config --file VERSION.cfg release.version {version}" }),
        )
        .await;
        for epic in ["base", "auth"] {
            rig.call(
                Method::POST,
                "/v1/projects/DEMO/chains",
                json!({ "name": epic }),
            )
            .await;
        }
        for (epic, title) in [("base", "First"), ("auth", "Second")] {
            rig.call(
                Method::POST,
                &format!("/v1/chains/{epic}/tickets"),
                json!({ "title": title }),
            )
            .await;
        }
        rig.hand_in("DEMO-1", "a.txt").await;
        rig.accept("DEMO-1").await;
        assert_eq!(rig.status("DEMO-1").await, "merged");
        rig
    }

    async fn send(&self, method: Method, uri: &str, body: Value) -> (StatusCode, Value) {
        let req = Request::builder()
            .method(method)
            .uri(uri)
            .header("authorization", format!("Bearer {TOKEN}"))
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .expect("request");
        let resp = self.app.clone().oneshot(req).await.expect("response");
        self.store.settled().await;
        let status = resp.status();
        let bytes = resp.into_body().collect().await.expect("body").to_bytes();
        let v = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, v)
    }

    async fn call(&self, method: Method, uri: &str, body: Value) -> Value {
        let (status, v) = self.send(method, uri, body).await;
        assert!(status.is_success(), "{uri}: {status} {v}");
        v
    }

    async fn card(&self, key: &str) -> Value {
        self.call(Method::GET, &format!("/v1/tickets/{key}"), Value::Null)
            .await
    }

    async fn status(&self, key: &str) -> String {
        let card = self.card(key).await;
        card["ticket"]["status"]
            .as_str()
            .unwrap_or_default()
            .to_owned()
    }

    /// The card's work is committed on its ship and handed in; its check has run.
    async fn hand_in(&self, key: &str, file: &str) {
        let card = self.card(key).await;
        let ship = PathBuf::from(card["ship"]["path"].as_str().expect("a ship"));
        commit_file(&ship, file);
        let summary = toknila_core::handin::from_plain("done");
        self.call(
            Method::POST,
            &format!("/v1/tickets/{key}/events"),
            json!({ "event": "finished", "summary": summary }),
        )
        .await;
        let card = self.card(key).await;
        assert_eq!(card["ticket"]["review"]["turn"]["by"], "person", "{card}");
    }

    async fn accept(&self, key: &str) {
        self.call(
            Method::POST,
            &format!("/v1/tickets/{key}/actions/accept"),
            json!({}),
        )
        .await;
    }

    async fn run(&self) -> Value {
        self.call(Method::GET, "/v1/releases/0.3.8/run", Value::Null)
            .await
    }

    /// Polls the release run until `done` says so, or fails at the deadline.
    async fn run_until(&self, what: &str, done: impl Fn(&Value) -> bool) -> Value {
        let until = Instant::now() + DEADLINE;
        loop {
            let run = self.run().await;
            if done(&run) {
                return run;
            }
            assert!(Instant::now() < until, "the run never got to {what}: {run}");
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    async fn status_until(&self, key: &str, want: &str) {
        let until = Instant::now() + DEADLINE;
        while self.status(key).await != want {
            assert!(Instant::now() < until, "{key} never got {want}");
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }
}

fn step<'a>(run: &'a Value, name: &str) -> &'a Value {
    run["steps"]
        .as_array()
        .and_then(|s| s.iter().find(|s| s["step"] == name))
        .unwrap_or(&Value::Null)
}

#[tokio::test]
async fn a_card_accepted_during_a_release_merges_after_its_tag_and_the_tag_needs_no_second_try() {
    let rig = Rig::new().await;
    rig.hand_in("DEMO-2", "b.txt").await;
    rig.call(
        Method::POST,
        "/v1/chains/base/release",
        json!({ "version": "0.3.8" }),
    )
    .await;
    rig.run_until("its Check", |r| step(r, "check")["state"] == "running")
        .await;

    // The person accepts while the release's Check runs.
    rig.accept("DEMO-2").await;
    let card = rig.card("DEMO-2").await;
    assert_eq!(card["ticket"]["review"]["turn"]["by"], "merging", "{card}");
    assert_eq!(
        card["review"]["merge_wait"],
        json!({ "for": "release", "version": "0.3.8" }),
        "{card}"
    );
    let list = rig.call(Method::GET, "/v1/tickets", Value::Null).await;
    let item = list
        .as_array()
        .and_then(|all| all.iter().find(|i| i["ticket"]["key"] == "DEMO-2"))
        .expect("DEMO-2 is listed");
    assert_eq!(item["merge_wait"]["for"], "release", "{item}");

    let run = rig.run_until("its end", |r| r["state"] != "running").await;
    assert_eq!(run["state"], "done", "the Tag took one try: {run}");
    assert_eq!(run["error"], Value::Null, "{run}");
    rig.status_until("DEMO-2", "merged").await;

    // The tag is on develop and holds the bump, not the card; the card merged after it, once
    // checked again against the bumped main.
    let (tagged, _) = git(
        &rig.repo,
        &["merge-base", "--is-ancestor", "v0.3.8", "develop"],
    );
    assert!(tagged, "the tag is on develop");
    let (has_card, _) = git(&rig.repo, &["cat-file", "-e", "v0.3.8:b.txt"]);
    assert!(!has_card, "the card merged after the tag");
    assert_eq!(git_ok(&rig.repo, &["show", "develop:b.txt"]), "b.txt");
    let rounds = rig.card("DEMO-2").await["ticket"]["review"]["rounds"].clone();
    let said: Vec<&str> = rounds
        .as_array()
        .map(|r| r.iter().filter_map(|x| x["comment"].as_str()).collect())
        .unwrap_or_default();
    assert!(
        said.contains(&"main moved since the check: 1 commit"),
        "{said:?}"
    );
    assert_eq!(
        checks(&rig.repo),
        4,
        "two hand-ins, the release's Check, one re-check"
    );
}

#[tokio::test]
async fn the_release_check_waits_for_its_turn_in_the_machine_check_line() {
    let rig = Rig::new().await;
    let before = checks(&rig.repo);
    // Another check holds the machine's one turn.
    let line = rig.store.check_line();
    let (held_tx, held) = std::sync::mpsc::channel();
    let (free, free_rx) = std::sync::mpsc::channel::<()>();
    let holder = std::thread::spawn(move || {
        let turn = line.enter(toknila_core::TicketId::new(), CheckKind::HandIn);
        held_tx.send(()).ok();
        free_rx.recv_timeout(DEADLINE).ok();
        drop(turn);
    });
    held.recv_timeout(DEADLINE)
        .expect("the other check holds the turn");

    rig.call(
        Method::POST,
        "/v1/chains/base/release",
        json!({ "version": "0.3.8" }),
    )
    .await;
    let run = rig
        .run_until("a wait in the check line", |r| {
            step(r, "check")["wait"].is_string()
        })
        .await;
    assert_eq!(
        step(&run, "check")["wait"],
        "waits for its turn (#1 in the check line)",
        "{run}"
    );
    assert_eq!(checks(&rig.repo), before, "the release's check has not run");

    free.send(()).ok();
    holder.join().expect("the other check ended");
    let run = rig.run_until("its end", |r| r["state"] != "running").await;
    assert_eq!(run["state"], "done", "{run}");
    assert_eq!(checks(&rig.repo), before + 1);
}
