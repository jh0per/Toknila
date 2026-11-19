//! "Agents at once" is a limit that holds, through the full router, the real store, a real git
//! ship and the real shell checker. With the limit at 1, two simulated stories complete; one is
//! sent back by its reviewer while the other runs, waits, says so, and goes on when the other
//! leaves. The journal is replayed afterwards: at no recorded moment did two stories hold an
//! agent. After a restart with real processes (the workspace's `fake-agent`, never a real agent)
//! the replacements start one per free slot.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::http::{Method, Request};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tokio::sync::Notify;
use toknila_daemon::runner::Runners;
use toknila_daemon::server::{AppState, router};
use toknila_daemon::shipyard::GitShipyard;
use toknila_daemon::store::Store;
use tower::ServiceExt;

const TOKEN: &str = "t";
const AGENT: &str = env!("CARGO_BIN_EXE_fake-agent");
const WAIT: Duration = Duration::from_secs(30);
const SENT_BACK: &str = "sent back; goes on when an agent is free";
const REVIEWER_DUE: &str = "the reviewer is due; it starts when an agent is free";

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

fn commit_file(dir: &Path, name: &str) {
    std::fs::write(dir.join(name), "x\n").expect("write");
    git(dir, &["add", name]);
    git(dir, &["commit", "--quiet", "-m", name]);
}

/// A repository on `develop` with one commit.
fn repo(root: &Path) -> PathBuf {
    let repo = root.join("repo");
    std::fs::create_dir_all(&repo).expect("mkdir");
    git(&repo, &["init", "--quiet"]);
    git(&repo, &["symbolic-ref", "HEAD", "refs/heads/develop"]);
    commit_file(&repo, "README.md");
    repo
}

/// One daemon over the database in `root`: its store, its runners and its router. The
/// supervisor is not started: a process that dies is recorded nowhere.
struct Daemon {
    app: axum::Router,
    store: Store,
    runners: Arc<Runners>,
    _ended: tokio::sync::mpsc::UnboundedReceiver<toknila_app::runner::RunEnded>,
}

impl Daemon {
    fn start(root: &Path) -> Daemon {
        let profiles = root.join("p");
        std::fs::create_dir_all(&profiles).expect("profiles");
        std::fs::write(
            profiles.join("holder.toml"),
            format!("command = ['{AGENT}', 'print', 'holding', 'hold']\n"),
        )
        .expect("profile");
        let (runners, ended) = Runners::new(root.join("runs"), profiles.clone());
        let store = Store::open(&root.join("t.db"))
            .expect("store")
            .with_runner(runners.clone())
            .with_shipyard(Arc::new(GitShipyard::new(root.join("ships"))))
            .with_merger(Arc::new(toknila_daemon::github::Mergers::new()));
        let app = router(AppState {
            store: store.clone(),
            token: TOKEN.into(),
            shutdown: Arc::new(Notify::new()),
            profiles,
            runners: runners.clone(),
            tools: Arc::new(toknila_daemon::tools::SystemTools),
            usage: Arc::new(toknila_daemon::usage::Usage::disabled()),
            home: root.to_owned(),
            ships: root.join("ships"),
            trash: Arc::new(toknila_daemon::trash::NoTrash),
            pdf_renderer: Arc::new(toknila_daemon::designs::thumb::NoRenderer),
        });
        Daemon {
            app,
            store,
            runners,
            _ended: ended,
        }
    }

    /// Sends a request and waits for the work its command queued (the check, git) to settle.
    async fn send(&self, method: Method, uri: &str, body: Value) -> Value {
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
        let v: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        assert!(status.is_success(), "{uri}: {status} {v}");
        v
    }

    async fn post(&self, uri: &str, body: Value) -> Value {
        self.send(Method::POST, uri, body).await
    }

    async fn card(&self, key: &str) -> Value {
        self.send(Method::GET, &format!("/v1/tickets/{key}"), Value::Null)
            .await
    }

    /// The stories that hold an agent now (`used` of the only project).
    async fn used(&self) -> u64 {
        let agents = self.send(Method::GET, "/v1/agents", Value::Null).await;
        agents[0]["used"].as_u64().expect("used")
    }

    /// Polls until `ok` holds of the card `key`, with a deadline.
    async fn until(&self, key: &str, what: &str, ok: impl Fn(&Value) -> bool) -> Value {
        let until = Instant::now() + WAIT;
        loop {
            let c = self.card(key).await;
            if ok(&c) {
                return c;
            }
            assert!(Instant::now() < until, "{key}: no {what}: {}", c["ticket"]);
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }
}

/// `status`, the queue words and whether its agent waits to start, of a card.
fn state(c: &Value) -> (String, Value, bool) {
    let t = &c["ticket"];
    let waits = !t["agent_paused"].is_null() || t["review"]["reviewer_waits"] == true;
    let status = t["status"].as_str().unwrap_or_default().to_owned();
    (status, c["queue_text"].clone(), waits)
}

/// The simulated worker of `key` commits `file` in its ship and hands in.
async fn hand_in(d: &Daemon, key: &str, file: &str) {
    let card = d.card(key).await;
    let ship = card["ship"]["path"].as_str().expect("the card has a ship");
    commit_file(Path::new(ship), file);
    let summary = toknila_core::handin::from_plain("done");
    d.post(
        &format!("/v1/tickets/{key}/events"),
        json!({ "event": "finished", "summary": summary }),
    )
    .await;
}

/// The simulated reviewer of `key` gives its verdict on the card's one acceptance item.
async fn verdict(d: &Daemon, key: &str, verdict: &str, outcome: &str) {
    d.post(
        &format!("/v1/tickets/{key}/events"),
        json!({ "event": "review_verdict", "verdict": verdict, "comment": "see the item",
                "items": [{ "item": 1, "outcome": outcome, "note": "the empty list" }] }),
    )
    .await;
}

/// Replays the journal: after every ticket change, the stories holding an agent (core's rule on
/// each snapshot) are at most `limit`. Returns how many snapshots it read.
async fn never_more_than(d: &Daemon, limit: usize) -> usize {
    let events = d.send(Method::GET, "/v1/events?since=0", Value::Null).await;
    let mut holding = std::collections::HashMap::new();
    let mut seen = 0;
    for e in events.as_array().expect("events") {
        let snapshot = e["payload"]["ticket"].clone();
        let Ok(t) = serde_json::from_value::<toknila_core::Ticket>(snapshot) else {
            continue;
        };
        seen += 1;
        holding.insert(t.key.to_string(), toknila_core::schedule::holds_slot(&t));
        let now: Vec<_> = holding
            .iter()
            .filter(|(_, h)| **h)
            .map(|(k, _)| k)
            .collect();
        assert!(
            now.len() <= limit,
            "event {}: {now:?} hold agents",
            e["seq"]
        );
    }
    seen
}

#[tokio::test]
async fn with_one_slot_a_story_sent_back_by_its_reviewer_while_the_other_runs_waits_and_both_merge()
{
    let tmp = tempfile::tempdir().expect("tmp");
    let repo = repo(tmp.path());
    let d = Daemon::start(tmp.path());
    d.post(
        "/v1/projects",
        json!({ "key": "DEMO", "repo_path": repo.to_string_lossy(), "default_profile": "sim",
                "simulated": true, "check_command": "git rev-parse HEAD", "max_parallel": 1,
                "default_review_mode": "agent" }),
    )
    .await;
    d.post("/v1/projects/DEMO/chains", json!({ "name": "auth" }))
        .await;
    for title in ["Login", "Profile"] {
        d.post(
            "/v1/chains/auth/tickets",
            json!({ "title": title, "prompt": "Do it.", "acceptance": ["It works"] }),
        )
        .await;
    }
    let (a, b) = ("DEMO-1", "DEMO-2");
    assert_eq!(state(&d.card(a).await).0, "running");
    assert_eq!(state(&d.card(b).await).1, "queued #1");

    // A hands in. While its check runs no agent of A works: B takes the slot. The check passes
    // and A's reviewer is due, but B holds the only agent: the reviewer waits.
    hand_in(&d, a, "login.txt").await;
    let ca = d.card(a).await;
    assert_eq!(
        state(&ca),
        ("review".to_owned(), json!(REVIEWER_DUE), true),
        "{}",
        ca["ticket"]["review"]
    );
    assert_eq!(
        state(&d.card(b).await),
        ("running".to_owned(), Value::Null, false)
    );
    assert_eq!(d.used().await, 1);

    // A's reviewer sends it back while B runs: A waits for B, and says so.
    verdict(&d, a, "request_changes", "fail").await;
    let ca = d.card(a).await;
    assert_eq!(state(&ca), ("running".to_owned(), json!(SENT_BACK), true));
    assert_eq!(ca["ticket"]["agent_paused"]["go_on"]["why"], "sent_back");
    assert_eq!(state(&d.card(b).await).0, "running");
    assert_eq!(d.used().await, 1);

    // B hands in: the story sent back goes on, first in line; B's reviewer, due after its check,
    // waits for A.
    hand_in(&d, b, "profile.txt").await;
    assert_eq!(
        state(&d.card(a).await),
        ("running".to_owned(), Value::Null, false)
    );
    assert_eq!(
        state(&d.card(b).await),
        ("review".to_owned(), json!(REVIEWER_DUE), true)
    );
    assert_eq!(d.used().await, 1);

    // A hands in again: B's reviewer gets the slot, A's waits; B's approval merges it and A's
    // reviewer starts; A's approval merges A.
    hand_in(&d, a, "login-2.txt").await;
    assert_eq!(
        state(&d.card(b).await),
        ("review".to_owned(), Value::Null, false)
    );
    assert!(state(&d.card(a).await).2, "A's reviewer waits");
    verdict(&d, b, "approve", "pass").await;
    d.until(b, "merge", |c| c["ticket"]["status"] == "merged")
        .await;
    assert_eq!(
        state(&d.card(a).await),
        ("review".to_owned(), Value::Null, false)
    );
    verdict(&d, a, "approve", "pass").await;
    d.until(a, "merge", |c| c["ticket"]["status"] == "merged")
        .await;
    assert_eq!(d.used().await, 0);

    // At no moment the journal recorded did two stories hold an agent.
    let seen = never_more_than(&d, 1).await;
    assert!(seen > 10, "the replay read the snapshots: {seen}");
}

#[tokio::test]
async fn after_a_restart_the_lost_agents_come_back_one_per_free_slot() {
    let tmp = tempfile::tempdir().expect("tmp");
    let repo = repo(tmp.path());
    let one = Daemon::start(tmp.path());
    one.post(
        "/v1/projects",
        json!({ "key": "DEMO", "repo_path": repo.to_string_lossy(), "default_profile": "holder" }),
    )
    .await;
    one.post("/v1/projects/DEMO/chains", json!({ "name": "auth" }))
        .await;
    for title in ["Card 1", "Card 2"] {
        one.post(
            "/v1/chains/auth/tickets",
            json!({ "title": title, "prompt": "Do it.", "profile": "holder" }),
        )
        .await;
    }
    let ids = [one.card("DEMO-1").await, one.card("DEMO-2").await].map(|c| {
        c["ticket"]["id"]
            .as_str()
            .expect("id")
            .parse()
            .expect("ticket id")
    });
    let until = Instant::now() + WAIT;
    while ids.iter().any(|id| !one.runners.live().contains(id)) {
        assert!(
            Instant::now() < until,
            "two agents start under the default limit"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    // The limit is lowered under two working agents: nobody is stopped.
    one.send(
        Method::PATCH,
        "/v1/projects/DEMO",
        json!({ "max_parallel": 1 }),
    )
    .await;
    assert_eq!(one.used().await, 2);

    // The update: the daemon and its processes are gone.
    one.runners.stop_all();
    one.store.settled().await;
    drop(one);
    let two = Daemon::start(tmp.path());
    let report = toknila_daemon::recovery::recover(&two.store)
        .await
        .expect("recover");
    assert_eq!(report.lost, ["DEMO-1", "DEMO-2"]);
    two.until("DEMO-1", "a Running card with its agent", |c| {
        c["ticket"]["status"] == "running" && c["ticket"]["agent_paused"].is_null()
    })
    .await;
    let second = two.card("DEMO-2").await;
    let words = "the app restarted; goes on when an agent is free";
    assert_eq!(state(&second), ("running".to_owned(), json!(words), true));
    assert_eq!(
        two.runners.live(),
        vec![ids[0]],
        "one agent at a limit of 1"
    );
    assert_eq!(two.used().await, 1);

    // The first leaves: the second's agent comes back.
    two.post(
        "/v1/tickets/DEMO-1/actions/cancel",
        json!({ "text": "not needed" }),
    )
    .await;
    two.until("DEMO-2", "its agent back", |c| {
        c["ticket"]["status"] == "running" && c["ticket"]["agent_paused"].is_null()
    })
    .await;
    let until = Instant::now() + WAIT;
    while two.runners.live() != vec![ids[1]] {
        assert!(Instant::now() < until, "live: {:?}", two.runners.live());
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(two.used().await, 1);
    two.runners.stop_all();
    two.store.settled().await;
}
