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
    /// How many runs are inside `run` now, and the most there ever were at once.
    active: AtomicUsize,
    most_active: AtomicUsize,
    release: Mutex<Option<mpsc::Receiver<()>>>,
}

impl SlowChecker {
    fn holding(hold: Duration) -> Arc<Self> {
        Arc::new(Self {
            hold,
            runs: AtomicUsize::new(0),
            active: AtomicUsize::new(0),
            most_active: AtomicUsize::new(0),
            release: Mutex::new(None),
        })
    }

    /// One that holds until the returned sender sends (or drops).
    fn stuck() -> (Arc<Self>, mpsc::Sender<()>) {
        let (tx, rx) = mpsc::channel();
        let c = Arc::new(Self {
            hold: Duration::ZERO,
            runs: AtomicUsize::new(0),
            active: AtomicUsize::new(0),
            most_active: AtomicUsize::new(0),
            release: Mutex::new(Some(rx)),
        });
        (c, tx)
    }
}

impl Checker for SlowChecker {
    fn run(&self, _spec: &CheckSpec) -> Result<CheckResult, CheckError> {
        self.runs.fetch_add(1, Ordering::SeqCst);
        let now = self.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.most_active.fetch_max(now, Ordering::SeqCst);
        let held = self.release.lock().expect("lock").take();
        match held {
            Some(rx) => {
                let _ = rx.recv();
            }
            None => std::thread::sleep(self.hold),
        }
        self.active.fetch_sub(1, Ordering::SeqCst);
        Ok(CheckResult {
            success: true,
            code: Some(0),
            timed_out: false,
            output: "fine".to_owned(),
        })
    }
}

fn app_for(store: &Store, home: &Path) -> axum::Router {
    let (runners, _) = toknila_daemon::runner::Runners::new(home.join("runs"), home.join("p"));
    router(AppState {
        store: store.clone(),
        token: TOKEN.into(),
        shutdown: Arc::new(Notify::new()),
        profiles: home.join("p"),
        runners,
        tools: Arc::new(toknila_daemon::tools::SystemTools),
        usage: Arc::new(toknila_daemon::usage::Usage::disabled()),
        home: home.to_owned(),
        ships: home.join("ships"),
        trash: Arc::new(toknila_daemon::trash::NoTrash),
        pdf_renderer: Arc::new(toknila_daemon::designs::thumb::NoRenderer),
    })
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
        let app = app_for(&store, tmp.path());
        let mut rig = Rig {
            _tmp: tmp,
            app,
            store,
            ships: Vec::new(),
        };
        rig.send(
            Method::POST,
            "/v1/projects",
            json!({ "key": "DEMO", "repo_path": repo.to_string_lossy(), "check_command": "true",
                "default_profile": "sim", "simulated": true }),
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
                json!({ "event": "finished", "summary": toknila_core::handin::from_plain("done") })
                    .to_string(),
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

/// The request is dropped on the writer thread right after the commit, before its answer is
/// sent: the earliest moment there is, with no polling in between.
#[tokio::test]
async fn a_request_dropped_the_moment_its_hand_in_commits_still_has_its_check_recorded() {
    let checker = SlowChecker::holding(Duration::from_millis(200));
    let rig = Rig::new(checker.clone(), 1).await;
    let slot: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>> = Arc::new(Mutex::new(None));
    let seen = slot.clone();
    rig.store.on_committed(Some(Arc::new(move || {
        let until = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(h) = seen.lock().expect("lock").take() {
                h.abort();
                return;
            }
            if Instant::now() > until {
                return;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    })));
    *slot.lock().expect("lock") = Some(rig.hand_in(1));
    let until = Instant::now() + Duration::from_secs(10);
    while slot.lock().expect("lock").is_some() {
        assert!(Instant::now() < until, "the hand-in never committed");
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
    rig.store.on_committed(None);
    rig.store.settled().await;
    assert_eq!(checker.runs.load(Ordering::SeqCst), 1);
    let card = rig.card(1).await;
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
    // The card was never given a result: the next start queues its check again.
    assert_eq!(rig.awaiting(1).await, json!({ "by": "check" }));
    release.send(()).expect("release");
    rig.store.settled().await;
    assert_eq!(
        rig.awaiting(1).await,
        json!({ "by": "check" }),
        "a result that comes after the shutdown began is not recorded"
    );
}

/// Waits until `check` holds for card `n` (polling its detail), or fails the test.
async fn until_card(rig: &Rig, n: usize, what: &str, check: impl Fn(&Value) -> bool) {
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        let card = rig.card(n).await;
        if check(&card) {
            return;
        }
        assert!(Instant::now() < until, "card {n}: {what}: {card}");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

#[tokio::test]
async fn two_hand_ins_at_once_run_their_checks_one_after_the_other_and_the_second_says_it_waits() {
    let (checker, release) = SlowChecker::stuck();
    let rig = Rig::new(checker.clone(), 2).await;
    let first = rig.hand_in(1);
    until_card(&rig, 1, "its check runs", |c| {
        c["review"]["check_queue"] == json!(0)
    })
    .await;
    let second = rig.hand_in(2);
    first.await.expect("answered");
    second.await.expect("answered");
    until_card(&rig, 2, "it waits for the first", |c| {
        c["review"]["check_queue"] == json!(1)
    })
    .await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(
        checker.runs.load(Ordering::SeqCst),
        1,
        "the second check did not start while the first is held"
    );
    assert_eq!(
        rig.awaiting(2).await,
        json!({ "by": "check" }),
        "still in the Check turn"
    );
    release.send(()).expect("release");
    rig.store.settled().await;
    assert_eq!(checker.runs.load(Ordering::SeqCst), 2);
    assert_eq!(
        checker.most_active.load(Ordering::SeqCst),
        1,
        "never two checks at once"
    );
    for n in 1..=2 {
        let card = rig.card(n).await;
        assert_eq!(card["review"]["rounds"][0]["verdict"], "approve", "{card}");
        assert!(card["review"]["check_queue"].is_null(), "{card}");
    }
}

#[tokio::test]
async fn a_check_that_waited_when_the_daemon_stopped_runs_after_the_restart() {
    let (checker, release) = SlowChecker::stuck();
    let rig = Rig::new(checker.clone(), 2).await;
    // Hand-in order is commit order: the second is handed in once the first has committed.
    let first = rig.hand_in(1);
    until_card(&rig, 1, "its check runs", |c| {
        c["review"]["check_queue"] == json!(0)
    })
    .await;
    let second = rig.hand_in(2);
    first.await.expect("answered");
    second.await.expect("answered");
    until_card(&rig, 2, "it waits for the first", |c| {
        c["review"]["check_queue"] == json!(1)
    })
    .await;
    // The daemon is gone with the line it held: a second one starts on the same database.
    let again = SlowChecker::holding(Duration::from_millis(100));
    let tmp = rig._tmp.path();
    let store = Store::open(&tmp.join("t.db"))
        .expect("reopen")
        .with_shipyard(Arc::new(GitShipyard::new(tmp.join("ships"))))
        .with_merger(Arc::new(toknila_daemon::github::Mergers::new()))
        .with_checker(again.clone());
    assert!(
        store.check_line().snapshot().is_empty(),
        "the line starts empty"
    );
    let report = toknila_daemon::recovery::recover(&store)
        .await
        .expect("recover");
    assert_eq!(
        report.checks_resumed,
        ["DEMO-1", "DEMO-2"],
        "in hand-in order"
    );
    store.settled().await;
    let restarted = Rig {
        app: app_for(&store, tmp),
        store: store.clone(),
        ships: Vec::new(),
        _tmp: tempfile::tempdir().expect("unused"),
    };
    assert_eq!(
        again.runs.load(Ordering::SeqCst),
        2,
        "both checks ran after the restart"
    );
    assert_eq!(again.most_active.load(Ordering::SeqCst), 1);
    for n in 1..=2 {
        let card = restarted.card(n).await;
        assert_eq!(card["review"]["rounds"][0]["verdict"], "approve", "{card}");
    }
    release.send(()).expect("release the first daemon's check");
    rig.store.settled().await;
}

#[tokio::test]
async fn hand_ins_made_at_the_same_moment_line_up_one_runs_and_the_other_waits() {
    let (checker, release) = SlowChecker::stuck();
    let rig = Rig::new(checker.clone(), 2).await;
    let (a, b) = (rig.hand_in(1), rig.hand_in(2));
    a.await.expect("answered");
    b.await.expect("answered");
    let until = Instant::now() + Duration::from_secs(30);
    loop {
        let mut places = Vec::new();
        for n in 1..=2 {
            places.push(rig.card(n).await["review"]["check_queue"].to_string());
        }
        places.sort();
        if places == ["0", "1"] {
            break;
        }
        assert!(Instant::now() < until, "one runs, one waits: {places:?}");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    release.send(()).expect("release");
    rig.store.settled().await;
    assert_eq!(checker.runs.load(Ordering::SeqCst), 2);
    assert_eq!(checker.most_active.load(Ordering::SeqCst), 1);
}
