//! A first-run screen is a person's question and nothing else's, through the full router with a
//! real runner process (the workspace's `fake-agent`, never a real agent): an agent that shows
//! Claude Code's trust dialog and has not shown any sign of life puts its card in Waiting
//! (reason: its screen), is never nudged and never paused however long it waits, and the answer a
//! person chooses in the inbox reaches its terminal as the keys of that option.
//!
//! The fake agent's dialog is drawn as Claude Code draws it: "No, exit" first and highlighted.

use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::http::{Method, Request};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tokio::sync::Notify;
use toknila_core::Timestamp;
use toknila_daemon::runner::{Runners, silence_tick, supervise};
use toknila_daemon::server::{AppState, now, router};
use toknila_daemon::shipyard::GitShipyard;
use toknila_daemon::store::Store;
use tower::ServiceExt;

const TOKEN: &str = "t";
const AGENT: &str = env!("CARGO_BIN_EXE_fake-agent");
const WAIT: Duration = Duration::from_secs(30);

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

struct Rig {
    _tmp: tempfile::TempDir,
    app: axum::Router,
    store: Store,
    runners: Arc<Runners>,
}

impl Rig {
    /// A project with one card whose agent runs `steps` of the fake agent; the profile gives it
    /// one second to show life before it counts as waiting on its screen.
    async fn new(steps: &str) -> Rig {
        let tmp = tempfile::tempdir().expect("tmp");
        let repo = tmp.path().join("repo");
        std::fs::create_dir_all(&repo).expect("mkdir");
        git(&repo, &["init", "--quiet"]);
        git(&repo, &["symbolic-ref", "HEAD", "refs/heads/develop"]);
        std::fs::write(repo.join("README.md"), "hi\n").expect("write");
        git(&repo, &["add", "README.md"]);
        git(&repo, &["commit", "--quiet", "-m", "first"]);
        let dir = tmp.path().join("p");
        std::fs::create_dir_all(&dir).expect("profiles");
        std::fs::write(
            dir.join("first.toml"),
            format!("command = ['{AGENT}', {steps}]\nstartup_secs = 1\n[hooks]\nenabled = true\n"),
        )
        .expect("profile");
        let (runners, ended) = Runners::new(tmp.path().join("runs"), dir.clone());
        // A card that waits is paused after this, except one that waits on its screen.
        runners.set_pause_grace(Duration::from_millis(100));
        let store = Store::open(&tmp.path().join("t.db"))
            .expect("store")
            .with_runner(runners.clone())
            .with_shipyard(Arc::new(GitShipyard::new(tmp.path().join("ships"))))
            .with_merger(Arc::new(toknila_daemon::github::Mergers::new()));
        supervise(store.clone(), ended);
        let app = router(AppState {
            store: store.clone(),
            token: TOKEN.into(),
            shutdown: Arc::new(Notify::new()),
            profiles: dir,
            runners: runners.clone(),
            tools: Arc::new(toknila_daemon::tools::SystemTools),
            usage: Arc::new(toknila_daemon::usage::Usage::disabled()),
            home: tmp.path().to_owned(),
            ships: tmp.path().join("ships"),
            trash: Arc::new(toknila_daemon::trash::NoTrash),
            pdf_renderer: Arc::new(toknila_daemon::designs::thumb::NoRenderer),
        });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr = listener.local_addr().expect("address");
        runners.set_daemon_url(format!("http://{addr}"));
        let served = app.clone();
        tokio::spawn(async move { axum::serve(listener, served).await });
        let rig = Rig {
            _tmp: tmp,
            app,
            store,
            runners,
        };
        rig.send(
            Method::POST,
            "/v1/projects",
            json!({ "key": "DEMO", "repo_path": repo.to_string_lossy(), "max_parallel": 2,
                    "default_profile": "first" }),
        )
        .await;
        rig.send(
            Method::POST,
            "/v1/projects/DEMO/chains",
            json!({ "name": "auth" }),
        )
        .await;
        rig.send(
            Method::POST,
            "/v1/chains/auth/tickets",
            json!({ "title": "Card 1", "prompt": "Do it.", "profile": "first" }),
        )
        .await;
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
        let status = resp.status();
        let bytes = resp.into_body().collect().await.expect("body").to_bytes();
        let v: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        assert!(status.is_success(), "{uri}: {status} {v}");
        v
    }

    async fn card(&self) -> Value {
        self.send(Method::GET, "/v1/tickets/DEMO-1", Value::Null)
            .await["ticket"]
            .clone()
    }

    async fn output(&self) -> String {
        let id = self.card().await["id"]
            .as_str()
            .expect("id")
            .parse()
            .expect("ticket id");
        self.runners
            .read(id, toknila_core::AgentRole::Worker, None)
            .map(|c| String::from_utf8_lossy(&c.bytes).into_owned())
            .unwrap_or_default()
    }

    /// Polls, running the silence ticker each round (at `skew` ms after now), until `ok` holds of
    /// the card.
    async fn until(&self, what: &str, skew: i64, ok: impl Fn(&Value) -> bool) -> Value {
        let until = Instant::now() + WAIT;
        loop {
            silence_tick(&self.store, &self.runners, Timestamp(now().0 + skew)).await;
            let c = self.card().await;
            if ok(&c) {
                return c;
            }
            assert!(
                Instant::now() < until,
                "no {what}: {c}\n{}",
                self.output().await
            );
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    async fn journal(&self) -> Vec<Value> {
        self.send(Method::GET, "/v1/events?since=0&limit=10000", Value::Null)
            .await
            .as_array()
            .expect("events")
            .clone()
    }

    async fn question(&self) -> Value {
        let inbox = self
            .send(Method::GET, "/v1/projects/DEMO/inbox", Value::Null)
            .await;
        inbox
            .as_array()
            .expect("items")
            .iter()
            .find(|i| i["kind"] == "question")
            .cloned()
            .unwrap_or(Value::Null)
    }
}

fn waits_on_screen(c: &Value) -> bool {
    c["status"] == "waiting" && c["status_reason"]["kind"] == "screen"
}

#[tokio::test]
async fn a_trust_dialog_waits_for_a_person_is_never_nudged_and_the_chosen_answer_reaches_the_terminal()
 {
    let activity = r#"{"signal":"hook:post_tool_use"}"#;
    let rig = Rig::new(&format!(
        "'trust', 'post', '/tickets/{{ticket}}/activity', '{activity}', 'hold'"
    ))
    .await;
    // The card waits on its screen, in words the window and `tok ls` show.
    let c = rig
        .until("a card waiting on its screen", 0, waits_on_screen)
        .await;
    assert_eq!(
        c["status_reason"],
        json!({ "status": "waiting", "kind": "screen" })
    );
    // The inbox row shows the screen and offers the dialog's two answers.
    let q = rig.question().await;
    assert_eq!(q["answers"], json!(["yes", "no"]), "{q}");
    let lines = q["lines"].to_string();
    assert!(lines.contains("Yes, I trust this folder") && lines.contains("No, exit"));
    // Silent for 200 s by the daemon's clock: the card is neither nudged nor paused, and the
    // dialog received nothing (a nudge would have answered it with "No, exit").
    tokio::time::sleep(Duration::from_millis(300)).await;
    for _ in 0..3 {
        silence_tick(&rig.store, &rig.runners, Timestamp(now().0 + 200_000)).await;
    }
    let c = rig.card().await;
    assert!(waits_on_screen(&c), "{c}");
    assert!(
        c["agent_paused"].is_null(),
        "the process holds the question"
    );
    assert!(!rig.output().await.contains("trust="), "nothing typed");
    let said = rig
        .journal()
        .await
        .iter()
        .map(|e| format!("{}.{}", e["entity"], e["kind"]))
        .collect::<Vec<_>>();
    assert!(
        !said.iter().any(|k| k.contains("comment")),
        "no nudge was written: {said:?}"
    );
    let raised = rig
        .journal()
        .await
        .into_iter()
        .find(|e| e["kind"] == "question_raised")
        .expect("question_raised");
    assert_eq!(raised["payload"]["kind"], "screen");
    let words = raised["payload"]["text"].as_str().expect("words");
    assert!(
        words.starts_with("the agent's screen asks: Do you trust"),
        "{words}"
    );
    // Yes: the daemon types Down and Enter, the agent goes on, and the question clears.
    rig.send(
        Method::POST,
        "/v1/tickets/DEMO-1/runner/answer",
        json!({ "answer": "yes" }),
    )
    .await;
    let out = rig
        .until("the card running again", 0, |c| c["status"] == "running")
        .await;
    assert!(out["status_reason"].is_null(), "{out}");
    // The card runs as soon as the keys are sent; the agent echoes them a moment later.
    let until = Instant::now() + WAIT;
    while !rig.output().await.contains("trust=yes") {
        assert!(
            Instant::now() < until,
            "the agent never echoed the answer:\n{}",
            rig.output().await
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let kinds = rig.journal().await;
    let answered = kinds
        .iter()
        .find(|e| e["kind"] == "question_answered")
        .expect("question_answered");
    assert_eq!(answered["payload"]["answer"], "yes");
    assert!(rig.question().await.is_null(), "the question is gone");
}

#[tokio::test]
async fn no_exits_the_dialog_and_a_run_that_showed_life_is_nudged_as_before() {
    // No: the highlighted option, so Enter alone.
    let rig = Rig::new("'trust', 'hold'").await;
    rig.until("a card waiting on its screen", 0, waits_on_screen)
        .await;
    rig.send(
        Method::POST,
        "/v1/tickets/DEMO-1/runner/answer",
        json!({ "answer": "no" }),
    )
    .await;
    let until = Instant::now() + WAIT;
    while !rig.output().await.contains("trust=no") {
        assert!(Instant::now() < until, "{}", rig.output().await);
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    // Control: an agent that reported life and then is quiet for 200 s is nudged (the nudge is
    // typed into its terminal and answered there).
    let activity = r#"{"signal":"hook:post_tool_use"}"#;
    let alive = Rig::new(&format!(
        "'post', '/tickets/{{ticket}}/activity', '{activity}', 'echo', 'hold'"
    ))
    .await;
    alive
        .until("the card running", 0, |c| c["status"] == "running")
        .await;
    let until = Instant::now() + WAIT;
    loop {
        silence_tick(&alive.store, &alive.runners, Timestamp(now().0 + 200_000)).await;
        if alive.output().await.contains("got:") {
            break;
        }
        assert!(Instant::now() < until, "no nudge: {}", alive.output().await);
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
