//! The host work a command queues (a hand-in's check command, git, a merge) runs in a task the
//! request does not own: a client that gives up, or an agent that the same hand-in stops, never
//! loses the outcome. The check here is a fake that holds for a while.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::http::{Method, Request};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tokio::sync::Notify;
use toknila_app::{CheckError, CheckResult, CheckSpec, Checker};
use toknila_daemon::server::{AppState, router};
use toknila_daemon::shipyard::GitShipyard;
use toknila_daemon::store::Store;
use tower::ServiceExt;

const TOKEN: &str = "t";

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .output()
        .expect("git runs");
    assert!(out.status.success(), "git {args:?}");
}

/// A check that holds for `hold`, or until released, and passes.
#[derive(Debug)]
struct SlowChecker {
    hold: Duration,
    runs: AtomicUsize,
    release: Mutex<Option<mpsc::Receiver<()>>>,
}

impl SlowChecker {
    fn holding(hold: Duration) -> Arc<Self> {
        Arc::new(Self {
            hold,
            runs: AtomicUsize::new(0),
            release: Mutex::new(None),
        })
    }

    /// One that holds until the returned sender sends (or drops).
    fn stuck() -> (Arc<Self>, mpsc::Sender<()>) {
        let (tx, rx) = mpsc::channel();
        let c = Arc::new(Self {
            hold: Duration::ZERO,
            runs: AtomicUsize::new(0),
            release: Mutex::new(Some(rx)),
        });
        (c, tx)
    }
}

impl Checker for SlowChecker {
    fn run(&self, _spec: &CheckSpec) -> Result<CheckResult, CheckError> {
        self.runs.fetch_add(1, Ordering::SeqCst);
        match self.release.lock().expect("lock").take() {
            Some(rx) => {
                let _ = rx.recv();
            }
            None => std::thread::sleep(self.hold),
        }
        Ok(CheckResult {
            success: true,
            code: Some(0),
            timed_out: false,
            output: "fine".to_owned(),
        })
    }
}

struct Rig {
    _tmp: tempfile::TempDir,
    app: axum::Router,
    store: Store,
    ships: Vec<PathBuf>,
}

impl Rig {
    async fn new(checker: Arc<SlowChecker>, cards: usize) -> Rig {
        let tmp = tempfile::tempdir().expect("tmp");
        let repo = tmp.path().join("repo");
        std::fs::create_dir_all(&repo).expect("mkdir");
        git(&repo, &["init", "--quiet"]);
        git(&repo, &["symbolic-ref", "HEAD", "refs/heads/develop"]);
        std::fs::write(repo.join("README.md"), "hi\n").expect("write");
        git(&repo, &["add", "README.md"]);
        git(&repo, &["commit", "--quiet", "-m", "first"]);
        let store = Store::open(&tmp.path().join("t.db"))
            .expect("store")
            .with_shipyard(Arc::new(GitShipyard::new(tmp.path().join("ships"))))
            .with_merger(Arc::new(toknila_daemon::github::Mergers::new()))
            .with_checker(checker);
        let (runners, _) =
            toknila_daemon::runner::Runners::new(tmp.path().join("runs"), tmp.path().join("p"));
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
        });
        let mut rig = Rig {
            _tmp: tmp,
            app,
            store,
            ships: Vec::new(),
        };
        rig.send(
            Method::POST,
            "/v1/projects",
            json!({ "key": "DEMO", "repo_path": repo.to_string_lossy(), "check_command": "true" }),
        )
        .await;
        rig.send(
            Method::POST,
            "/v1/projects/DEMO/chains",
            json!({ "name": "auth" }),
        )
        .await;
        for n in 1..=cards {
            rig.send(
                Method::POST,
                "/v1/chains/auth/tickets",
                json!({ "title": format!("Card {n}"), "prompt": "Do it.", "acceptance": ["ok"] }),
            )
            .await;
        }
        rig.store.settled().await;
        for n in 1..=cards {
            let card = rig.card(n).await;
            let path = card["ship"]["path"].as_str().expect("the card has a ship");
            rig.ships.push(PathBuf::from(path));
        }
        rig
    }

    async fn send(&self, method: Method, uri: &str, body: Value) -> Value {
        let req = Request::builder()
            .method(method)
            .uri(uri)
            .header("authorization", format!("Bearer {TOKEN}"))
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .expect("request");
        let resp = self.app.clone().oneshot(req).await.expect("response");
        assert!(resp.status().is_success(), "{uri}: {}", resp.status());
        let bytes = resp.into_body().collect().await.expect("body").to_bytes();
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    }

    async fn card(&self, n: usize) -> Value {
        self.send(Method::GET, &format!("/v1/tickets/DEMO-{n}"), Value::Null)
            .await
    }

    /// The worker of card `n` commits and hands in; the request only starts here.
    fn hand_in(&self, n: usize) -> tokio::task::JoinHandle<()> {
        let ship = &self.ships[n - 1];
        std::fs::write(ship.join("w.txt"), "x\n").expect("write");
        git(ship, &["add", "w.txt"]);
        git(ship, &["commit", "--quiet", "-m", "w"]);
        let app = self.app.clone();
        let req = Request::builder()
            .method(Method::POST)
            .uri(format!("/v1/tickets/DEMO-{n}/events"))
            .header("authorization", format!("Bearer {TOKEN}"))
            .header("content-type", "application/json")
            .body(Body::from(
                json!({ "event": "finished", "summary": "done" }).to_string(),
            ))
            .expect("request");
        tokio::spawn(async move {
            let _ = app.oneshot(req).await;
        })
    }

    async fn awaiting(&self, n: usize) -> Value {
        self.card(n).await["review"]["awaiting"].clone()
    }
}

#[tokio::test]
async fn a_hand_in_whose_client_is_gone_still_has_its_check_recorded() {
    let checker = SlowChecker::holding(Duration::from_secs(1));
    let rig = Rig::new(checker.clone(), 1).await;
    let handed = rig.hand_in(1);
    // The client gives up as soon as the hand-in has committed.
    let until = Instant::now() + Duration::from_secs(10);
    while rig.awaiting(1).await != json!({ "by": "check" }) {
        assert!(Instant::now() < until, "the hand-in never committed");
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    handed.abort();
    let started = Instant::now();
    rig.store.settled().await;
    assert!(started.elapsed() < Duration::from_secs(10));
    assert_eq!(checker.runs.load(Ordering::SeqCst), 1);
    let card = rig.card(1).await;
    assert_ne!(card["review"]["awaiting"]["by"], "check", "{card}");
    assert_eq!(
        card["review"]["rounds"][0]["verdict"], "approve",
        "the check was recorded as a round: {card}"
    );
}

#[tokio::test]
async fn the_hand_in_is_answered_at_once_and_the_card_is_being_checked_meanwhile() {
    let checker = SlowChecker::holding(Duration::from_secs(1));
    let rig = Rig::new(checker, 1).await;
    let started = Instant::now();
    rig.hand_in(1).await.expect("answered");
    assert!(
        started.elapsed() < Duration::from_millis(800),
        "the request did not wait for the check: {:?}",
        started.elapsed()
    );
    assert_eq!(rig.awaiting(1).await, json!({ "by": "check" }));
    rig.store.settled().await;
    assert_ne!(rig.awaiting(1).await["by"], "check");
}

#[tokio::test]
async fn two_hand_ins_at_once_record_both_checks() {
    let checker = SlowChecker::holding(Duration::from_millis(400));
    let rig = Rig::new(checker.clone(), 2).await;
    let (a, b) = (rig.hand_in(1), rig.hand_in(2));
    a.await.expect("answered");
    b.await.expect("answered");
    rig.store.settled().await;
    assert_eq!(checker.runs.load(Ordering::SeqCst), 2);
    for n in 1..=2 {
        assert_ne!(rig.awaiting(n).await["by"], "check", "card {n}");
        let card = rig.card(n).await;
        assert_eq!(card["review"]["rounds"][0]["verdict"], "approve", "{card}");
    }
}

#[tokio::test]
async fn shutdown_does_not_wait_for_an_effect_that_never_ends() {
    let (checker, release) = SlowChecker::stuck();
    let rig = Rig::new(checker, 1).await;
    rig.hand_in(1).await.expect("answered");
    let started = Instant::now();
    let abandoned = rig.store.shutdown(Duration::from_millis(300)).await;
    assert!(started.elapsed() < Duration::from_secs(3));
    assert!(
        abandoned.len() == 1 && abandoned[0].contains("RunCheck"),
        "the stuck check is named: {abandoned:?}"
    );
    // The card was never given a result: the next start hands it to a person.
    assert_eq!(rig.awaiting(1).await, json!({ "by": "check" }));
    release.send(()).expect("release");
    rig.store.settled().await;
    assert_eq!(
        rig.awaiting(1).await,
        json!({ "by": "check" }),
        "a result that comes after the shutdown began is not recorded"
    );
}
