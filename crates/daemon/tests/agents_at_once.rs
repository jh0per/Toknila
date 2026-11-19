//! "Agents at once" is a limit that holds, through the full router, the real store, a real git
//! ship and the real shell checker. With the limit at 1, two simulated stories complete; one is
//! sent back by its reviewer while the other runs, waits, says so, and goes on when the other
//! leaves. The journal is replayed afterwards: at no recorded moment did two stories hold an
//! agent. With real processes (the workspace's `fake-agent`, never a real agent), counted on the
//! runner itself rather than by the rule: after a restart the replacements and a reviewer that was
//! due start one per free slot; a worker that exits by itself after its hand-in and a reviewer
//! that exits by itself after its verdict give their slot to the next story with nobody touching
//! the project; a story blocked by hand keeps its slot while its agent lives.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::http::{Method, Request};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tokio::sync::Notify;
use tokio::task::JoinHandle;
use toknila_app::runner::Runner as _;
use toknila_core::{AgentRole, TicketId};
use toknila_daemon::runner::{Runners, supervise};
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

/// The agents' profiles: `holder` works until killed, `waiter` until a line is typed to it,
/// `quitter` hands in when a line is typed and exits by itself; each `-reviewer` is its reviewer
/// (`quitter-reviewer` approves when a line is typed and exits by itself).
fn profiles(dir: &Path) {
    let summary = toknila_core::handin::from_plain("done");
    let finished = json!({ "event": "finished", "summary": summary }).to_string();
    let approve = json!({ "event": "review_verdict", "verdict": "approve", "comment": "fine",
        "items": [{ "item": 1, "outcome": "pass", "note": "it works" }] })
    .to_string();
    let events = "/tickets/{ticket}/events";
    for (name, steps) in [
        ("holder", "'print', 'holding', 'hold'".to_owned()),
        ("holder-reviewer", "'print', 'reviewing', 'hold'".to_owned()),
        ("waiter", "'print', 'ready', 'echo'".to_owned()),
        (
            "quitter",
            format!("'print', 'ready', 'echo', 'post', '{events}', '{finished}'"),
        ),
        (
            "quitter-reviewer",
            format!("'print', 'ready', 'echo', 'post', '{events}', '{approve}'"),
        ),
    ] {
        let toml = format!("command = ['{AGENT}', {steps}]\n");
        std::fs::write(dir.join(format!("{name}.toml")), toml).expect("profile");
    }
}

/// One daemon over the database in `root`: its store, its runners and its router. A `live` one
/// runs as the daemon does: the supervisor records every process end, and the agents report to a
/// listener. Without it a process that dies is recorded nowhere.
struct Daemon {
    app: axum::Router,
    store: Store,
    runners: Arc<Runners>,
    _ended: Option<tokio::sync::mpsc::UnboundedReceiver<toknila_app::runner::RunEnded>>,
    supervisor: Option<JoinHandle<()>>,
    server: Option<JoinHandle<()>>,
}

impl Drop for Daemon {
    fn drop(&mut self) {
        for task in [self.supervisor.take(), self.server.take()].into_iter().flatten() {
            task.abort();
        }
    }
}

impl Daemon {
    async fn start(root: &Path, live: bool) -> Daemon {
        let dir = root.join("p");
        std::fs::create_dir_all(&dir).expect("profiles");
        profiles(&dir);
        let (runners, ended) = Runners::new(root.join("runs"), dir.clone());
        let store = Store::open(&root.join("t.db"))
            .expect("store")
            .with_runner(runners.clone())
            .with_shipyard(Arc::new(GitShipyard::new(root.join("ships"))))
            .with_merger(Arc::new(toknila_daemon::github::Mergers::new()));
        let (supervisor, ended) = if live {
            (Some(supervise(store.clone(), runners.clone(), ended)), None)
        } else {
            (None, Some(ended))
        };
        let profiles = dir;
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
        let mut server = None;
        if live {
            // The agents report to a real listener, as hooks do.
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
                .await
                .expect("bind");
            let addr = listener.local_addr().expect("address");
            runners.set_daemon_url(format!("http://{addr}"));
            let served = app.clone();
            server = Some(tokio::spawn(async move {
                let _ = axum::serve(listener, served).await;
            }));
        }
        Daemon {
            app,
            store,
            runners,
            _ended: ended,
            supervisor,
            server,
        }
    }

    /// Ends as the daemon ends: no process end is recorded any more, every process is killed and
    /// gone, and the work queued is done. Nothing of the test is left running.
    async fn shutdown(mut self) {
        if let Some(supervisor) = self.supervisor.take() {
            supervisor.abort();
            let _ = supervisor.await;
        }
        self.runners.stop_all();
        let until = Instant::now() + WAIT;
        while !self.runners.live_runners().is_empty() {
            assert!(Instant::now() < until, "{:?}", self.runners.live_runners());
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        self.store.settled().await;
    }

    async fn id(&self, key: &str) -> TicketId {
        let card = self.card(key).await;
        let id = card["ticket"]["id"].as_str().expect("id");
        id.parse().expect("ticket id")
    }

    /// Polls until the tickets with a live process of `role` are exactly `ids`, with a deadline.
    async fn until_live(&self, role: AgentRole, ids: &[TicketId]) {
        let want: HashSet<TicketId> = ids.iter().copied().collect();
        let until = Instant::now() + WAIT;
        loop {
            let live: HashSet<TicketId> = self.runners.live_in(role).into_iter().collect();
            if live == want {
                return;
            }
            assert!(Instant::now() < until, "{role:?} live: {live:?}, not {want:?}");
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }

    /// Polls until the worker of `ticket` has printed `what`, with a deadline.
    async fn until_printed(&self, ticket: TicketId, what: &str) {
        let until = Instant::now() + WAIT;
        loop {
            let out = self.runners.read(ticket, AgentRole::Worker, None);
            let out = out.map(|c| String::from_utf8_lossy(&c.bytes).into_owned());
            let out = out.unwrap_or_default();
            if out.contains(what) {
                return;
            }
            assert!(Instant::now() < until, "no {what:?} in {out:?}");
            tokio::time::sleep(Duration::from_millis(25)).await;
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

/// Samples the runner every few milliseconds and keeps the most stories it saw with a live
/// process at once (a story's worker and reviewer are one): the processes themselves, not the
/// rule's answer.
struct Watch {
    most: Arc<AtomicUsize>,
    task: JoinHandle<()>,
}

impl Watch {
    fn new(runners: &Arc<Runners>) -> Watch {
        let most = Arc::new(AtomicUsize::new(0));
        let (seen, runners) = (most.clone(), runners.clone());
        let task = tokio::spawn(async move {
            loop {
                let live = runners.live_runners();
                let stories: HashSet<TicketId> = live.into_iter().map(|(t, _)| t).collect();
                seen.fetch_max(stories.len(), Ordering::SeqCst);
                tokio::time::sleep(Duration::from_millis(2)).await;
            }
        });
        Watch { most, task }
    }

    fn most(&self) -> usize {
        self.most.load(Ordering::SeqCst)
    }
}

impl Drop for Watch {
    fn drop(&mut self) {
        self.task.abort();
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
    let d = Daemon::start(tmp.path(), false).await;
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
    let one = Daemon::start(tmp.path(), false).await;
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
    let two = Daemon::start(tmp.path(), true).await;
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

    // The first leaves: once its stopped process is gone, the second's agent comes back.
    two.post(
        "/v1/tickets/DEMO-1/actions/cancel",
        json!({ "text": "not needed" }),
    )
    .await;
    two.until("DEMO-2", "its agent back", |c| {
        c["ticket"]["status"] == "running" && c["ticket"]["agent_paused"].is_null()
    })
    .await;
    two.until_live(AgentRole::Worker, &[ids[1]]).await;
    assert_eq!(two.used().await, 1);
    two.shutdown().await;
}

/// One slot and two stories, Login and Profile, whose agents are processes of `profile`,
/// reviewed as `mode` says.
async fn two_stories(d: &Daemon, repo: &Path, profile: &str, mode: &str) -> [TicketId; 2] {
    d.post(
        "/v1/projects",
        json!({ "key": "DEMO", "repo_path": repo.to_string_lossy(), "default_profile": profile,
                "max_parallel": 1, "default_review_mode": mode }),
    )
    .await;
    d.post("/v1/projects/DEMO/chains", json!({ "name": "auth" }))
        .await;
    for title in ["Login", "Profile"] {
        d.post(
            "/v1/chains/auth/tickets",
            json!({ "title": title, "prompt": "Do it.", "acceptance": ["It works"],
                    "profile": profile }),
        )
        .await;
    }
    [d.id("DEMO-1").await, d.id("DEMO-2").await]
}

/// The worker of `key` has `file` committed in its ship and is told to go on: a `quitter` hands
/// in and exits, a `waiter` exits.
async fn go(d: &Daemon, key: &str, id: TicketId, file: &str) {
    let card = d.card(key).await;
    let ship = card["ship"]["path"].as_str().expect("the card has a ship");
    commit_file(Path::new(ship), file);
    d.runners
        .write(id, AgentRole::Worker, b"go\r")
        .expect("type to the worker");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_worker_that_exits_by_itself_after_handing_in_gives_its_slot_to_the_next_story() {
    let tmp = tempfile::tempdir().expect("tmp");
    let repo = repo(tmp.path());
    let d = Daemon::start(tmp.path(), true).await;
    let watch = Watch::new(&d.runners);
    let [a, b] = two_stories(&d, &repo, "quitter", "human").await;
    d.until_printed(a, "ready").await;
    assert_eq!(state(&d.card("DEMO-2").await).1, "queued #1");
    // A's worker hands in and exits by itself; nobody touches the project after that.
    go(&d, "DEMO-1", a, "login.txt").await;
    d.until_live(AgentRole::Worker, &[b]).await;
    let ca = d.card("DEMO-1").await;
    assert_eq!(ca["ticket"]["status"], "review", "{}", ca["ticket"]);
    assert_eq!(state(&d.card("DEMO-2").await).0, "running");
    assert_eq!(watch.most(), 1, "never two stories with a live process");
    d.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_reviewer_that_exits_by_itself_after_its_verdict_gives_its_slot_to_the_next_story() {
    let tmp = tempfile::tempdir().expect("tmp");
    let repo = repo(tmp.path());
    let d = Daemon::start(tmp.path(), true).await;
    let watch = Watch::new(&d.runners);
    let [a, b] = two_stories(&d, &repo, "quitter", "both").await;
    d.until_printed(a, "ready").await;
    // A hands in. B is asked for only once A's reviewer works (during A's check no agent of A
    // works, and a story in line would take the slot then).
    d.send(
        Method::PATCH,
        "/v1/chains/auth",
        json!({ "autostart": false }),
    )
    .await;
    go(&d, "DEMO-1", a, "login.txt").await;
    d.until_live(AgentRole::Reviewer, &[a]).await;
    d.post("/v1/tickets/DEMO-2/actions/start", json!({})).await;
    assert_eq!(state(&d.card("DEMO-2").await).1, "queued #1");
    // The reviewer approves and exits by itself; a person has the turn; nobody touches the
    // project after that.
    let reviewer = |what: &str| {
        let out = d.runners.read(a, AgentRole::Reviewer, None);
        out.is_some_and(|c| String::from_utf8_lossy(&c.bytes).contains(what))
    };
    let until = Instant::now() + WAIT;
    while !reviewer("ready") {
        assert!(Instant::now() < until, "the reviewer never got ready");
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    d.runners
        .write(a, AgentRole::Reviewer, b"go\r")
        .expect("type to the reviewer");
    d.until_live(AgentRole::Worker, &[b]).await;
    let ca = d.card("DEMO-1").await;
    let review = &ca["ticket"]["review"];
    assert_eq!(review["turn"]["by"], "person", "{review}");
    assert!(review["note"].is_null(), "it gave its verdict: {review}");
    assert_eq!(state(&d.card("DEMO-2").await).0, "running");
    assert_eq!(watch.most(), 1, "never two stories with a live process");
    d.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_story_blocked_by_hand_keeps_its_slot_while_its_agent_lives() {
    let tmp = tempfile::tempdir().expect("tmp");
    let repo = repo(tmp.path());
    let d = Daemon::start(tmp.path(), true).await;
    let watch = Watch::new(&d.runners);
    let [a, b] = two_stories(&d, &repo, "waiter", "human").await;
    d.until_printed(a, "ready").await;
    d.post(
        "/v1/tickets/DEMO-1/actions/block",
        json!({ "text": "waiting for the key" }),
    )
    .await;
    // The block keeps A's agent, and its live process keeps the slot: B does not start.
    assert_eq!(d.card("DEMO-1").await["ticket"]["status"], "blocked");
    assert_eq!(d.runners.live(), vec![a]);
    assert_eq!(state(&d.card("DEMO-2").await).1, "queued #1");
    // A's process ends by itself: the slot is free and B starts; nobody touches the project.
    go(&d, "DEMO-1", a, "notes.txt").await;
    d.until_live(AgentRole::Worker, &[b]).await;
    assert_eq!(d.card("DEMO-1").await["ticket"]["status"], "blocked");
    assert_eq!(watch.most(), 1, "never two stories with a live process");
    d.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn after_a_restart_a_reviewer_that_was_due_starts_from_the_line_and_is_not_lost() {
    let tmp = tempfile::tempdir().expect("tmp");
    let repo = repo(tmp.path());
    let one = Daemon::start(tmp.path(), false).await;
    one.post(
        "/v1/projects",
        json!({ "key": "DEMO", "repo_path": repo.to_string_lossy(), "default_profile": "holder",
                "default_review_mode": "agent" }),
    )
    .await;
    one.post("/v1/projects/DEMO/chains", json!({ "name": "auth" }))
        .await;
    for title in ["Card 1", "Card 2"] {
        one.post(
            "/v1/chains/auth/tickets",
            json!({ "title": title, "prompt": "Do it.", "acceptance": ["It works"],
                    "profile": "holder" }),
        )
        .await;
    }
    let ids = [one.id("DEMO-1").await, one.id("DEMO-2").await];
    one.until_live(AgentRole::Worker, &ids).await;
    one.send(
        Method::PATCH,
        "/v1/projects/DEMO",
        json!({ "max_parallel": 1 }),
    )
    .await;
    // Card 1 hands in while Card 2 holds the only agent: its reviewer is due and waits.
    hand_in(&one, "DEMO-1", "card-1.txt").await;
    let c1 = one
        .until("DEMO-1", "a reviewer that is due", |c| {
            c["ticket"]["review"]["reviewer_waits"] == true
        })
        .await;
    assert_eq!(state(&c1), ("review".to_owned(), json!(REVIEWER_DUE), true));
    assert!(one.runners.live_in(AgentRole::Reviewer).is_empty());

    // The update: the daemon and its processes are gone.
    one.runners.stop_all();
    one.store.settled().await;
    drop(one);
    let two = Daemon::start(tmp.path(), true).await;
    let watch = Watch::new(&two.runners);
    let report = toknila_daemon::recovery::recover(&two.store)
        .await
        .expect("recover");
    assert!(report.reviews_lost.is_empty(), "{:?}", report.reviews_lost);
    assert_eq!(report.lost, ["DEMO-2"]);
    // Card 1's reviewer starts from the line, first; Card 2's agent waits for it.
    two.until_live(AgentRole::Reviewer, &[ids[0]]).await;
    let c1 = two.card("DEMO-1").await;
    assert_eq!(state(&c1), ("review".to_owned(), Value::Null, false));
    assert_eq!(c1["ticket"]["review"]["turn"]["by"], "agent");
    assert!(c1["ticket"]["review"]["note"].is_null(), "not sent to a person");
    let words = "the app restarted; goes on when an agent is free";
    let c2 = two.card("DEMO-2").await;
    assert_eq!(state(&c2), ("running".to_owned(), json!(words), true));
    assert!(two.runners.live().is_empty(), "no worker beside the reviewer");
    // The reviewer approves and leaves: Card 1 merges and Card 2's agent comes back.
    verdict(&two, "DEMO-1", "approve", "pass").await;
    two.until("DEMO-1", "merge", |c| c["ticket"]["status"] == "merged")
        .await;
    two.until_live(AgentRole::Worker, &[ids[1]]).await;
    assert_eq!(watch.most(), 1, "never two stories with a live process");
    two.shutdown().await;
}
