//! The loop the daemon closes, through the full router with a real git ship and the real shell
//! checker: a hand-in is checked by the daemon (no person involved when it fails), the reviewer
//! judges each acceptance item, every return is a round, and after the project's round limit the
//! card waits for the person with the whole history. The test plays the fake worker (commits in
//! the ship, says `finished`) and the fake reviewer (says `review_verdict`).

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tokio::sync::Notify;
use toknila_daemon::server::{AppState, router};
use toknila_daemon::shipyard::GitShipyard;
use toknila_daemon::store::Store;
use tower::ServiceExt;

const TOKEN: &str = "t";
/// Passes when `ok.txt` is committed in the ship, and fails with git's own message when not; the
/// same words on every platform's shell.
const CHECK: &str = "git ls-files --error-unmatch ok.txt";

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

fn commit_file(dir: &Path, name: &str, content: &str) {
    std::fs::write(dir.join(name), content).expect("write");
    git(dir, &["add", name]);
    git(dir, &["commit", "--quiet", "-m", name]);
}

struct Rig {
    _tmp: tempfile::TempDir,
    app: axum::Router,
    store: Store,
    ship: PathBuf,
    /// The person's checkout, on the base branch `develop`.
    repo: PathBuf,
    runners: Arc<toknila_daemon::runner::Runners>,
    _ended: tokio::sync::mpsc::UnboundedReceiver<toknila_app::runner::RunEnded>,
    ticket_id: Option<toknila_core::TicketId>,
}

impl Rig {
    async fn new(rounds: u32) -> Rig {
        Rig::with_check(rounds, Some(CHECK)).await
    }

    async fn with_check(rounds: u32, check: Option<&str>) -> Rig {
        Rig::build(rounds, check, false).await
    }

    /// A rig whose reviewer is the fake agent: it prints the kickoff it was started with and
    /// holds. Stop it with [`Rig::stop_reviewer`].
    async fn with_fake_reviewer() -> Rig {
        Rig::build(3, Some(CHECK), true).await
    }

    fn stop_reviewer(&self) {
        use toknila_app::runner::Runner;
        self.runners
            .stop_role(self.ticket(), toknila_core::AgentRole::Reviewer);
    }

    fn ticket(&self) -> toknila_core::TicketId {
        self.ticket_id.expect("the fake reviewer rig has a card")
    }

    /// What the fake reviewer printed so far.
    fn reviewer_output(&self) -> String {
        self.runners
            .read(self.ticket(), toknila_core::AgentRole::Reviewer, None)
            .map(|c| String::from_utf8_lossy(&c.bytes).into_owned())
            .unwrap_or_default()
    }

    async fn build(rounds: u32, check: Option<&str>, fake_reviewer: bool) -> Rig {
        let tmp = tempfile::tempdir().expect("tmp");
        let repo = tmp.path().join("repo");
        std::fs::create_dir_all(&repo).expect("mkdir");
        git(&repo, &["init", "--quiet"]);
        git(&repo, &["symbolic-ref", "HEAD", "refs/heads/develop"]);
        commit_file(&repo, "README.md", "hi\n");

        let (runners, ended) =
            toknila_daemon::runner::Runners::new(tmp.path().join("runs"), tmp.path().join("p"));
        let mut store = Store::open(&tmp.path().join("t.db"))
            .expect("store")
            .with_shipyard(Arc::new(GitShipyard::new(tmp.path().join("ships"))))
            .with_merger(Arc::new(toknila_daemon::github::Mergers::new()));
        if fake_reviewer {
            store = store.with_runner(runners.clone());
            std::fs::create_dir_all(tmp.path().join("p")).expect("profiles");
            let agent = env!("CARGO_BIN_EXE_fake-agent");
            std::fs::write(
                tmp.path().join("p").join("fake-reviewer.toml"),
                format!("command = ['{agent}', 'print', '{{kickoff}}', 'hold']\n"),
            )
            .expect("profile");
        }
        let app = router(AppState {
            store: store.clone(),
            token: TOKEN.into(),
            shutdown: Arc::new(Notify::new()),
            profiles: tmp.path().join("p"),
            runners: runners.clone(),
            tools: Arc::new(toknila_daemon::tools::SystemTools),
            usage: Arc::new(toknila_daemon::usage::Usage::disabled()),
            home: tmp.path().to_owned(),
            ships: tmp.path().join("ships"),
            trash: Arc::new(toknila_daemon::trash::NoTrash),
            pdf_renderer: Arc::new(toknila_daemon::designs::thumb::NoRenderer),
        });
        let mut rig = Rig {
            _tmp: tmp,
            app,
            store,
            ship: PathBuf::new(),
            repo: repo.clone(),
            runners,
            _ended: ended,
            ticket_id: None,
        };
        let project = rig
            .post(
                "/v1/projects",
                match check {
                    Some(c) => json!({ "key": "DEMO", "repo_path": repo.to_string_lossy(),
                        "check_command": c, "default_profile": "sim", "simulated": true }),
                    None => json!({ "key": "DEMO", "repo_path": repo.to_string_lossy(),
                        "default_profile": "sim", "simulated": true }),
                },
            )
            .await;
        // A new project reviews with both, three rounds of the loop.
        assert_eq!(project["default_review_mode"], "both");
        assert_eq!(project["max_review_rounds"], 3);
        let patched = rig
            .call(
                Method::PATCH,
                "/v1/projects/DEMO",
                json!({ "max_review_rounds": rounds }),
            )
            .await;
        assert_eq!(patched["max_review_rounds"], rounds);
        if fake_reviewer {
            rig.call(
                Method::PATCH,
                "/v1/projects/DEMO",
                json!({ "reviewer": "fake-reviewer" }),
            )
            .await;
        }
        rig.post("/v1/projects/DEMO/chains", json!({ "name": "auth" }))
            .await;
        rig.post(
            "/v1/chains/auth/tickets",
            json!({ "title": "Add login", "prompt": "Let people in.",
                    "acceptance": ["It logs in", "It is tidy"] }),
        )
        .await;
        let card = rig.card().await;
        rig.ship = PathBuf::from(card["ship"]["path"].as_str().expect("the card has a ship"));
        rig.ticket_id = card["ticket"]["id"].as_str().and_then(|id| id.parse().ok());
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
        // The effects of the command (the check, git) run after the answer; wait for them.
        self.store.settled().await;
        let status = resp.status();
        let bytes = resp.into_body().collect().await.expect("body").to_bytes();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }

    async fn call(&self, method: Method, uri: &str, body: Value) -> Value {
        let (status, v) = self.send(method, uri, body).await;
        assert!(status.is_success(), "{uri}: {status} {v}");
        v
    }

    async fn post(&self, uri: &str, body: Value) -> Value {
        self.call(Method::POST, uri, body).await
    }

    async fn card(&self) -> Value {
        self.call(Method::GET, "/v1/tickets/DEMO-1", Value::Null)
            .await
    }

    /// The fake worker hands in (after committing `file` in the ship).
    async fn finish(&self, file: &str) {
        commit_file(&self.ship, file, "x\n");
        self.post(
            "/v1/tickets/DEMO-1/events",
            json!({ "event": "finished", "summary": toknila_core::handin::from_plain("done") }),
        )
        .await;
    }

    /// The card once the daemon's work after the hand-in has settled: `done` holds.
    async fn settled(&self, what: &str, done: impl Fn(&Value) -> bool) -> Value {
        let until = Instant::now() + Duration::from_secs(30);
        loop {
            let card = self.card().await;
            if done(&card) {
                return card;
            }
            assert!(
                Instant::now() < until,
                "timed out waiting for {what}: {card}"
            );
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }
}

fn rounds(card: &Value) -> Vec<(String, String)> {
    card["review"]["rounds"]
        .as_array()
        .expect("rounds")
        .iter()
        .map(|r| {
            (
                r["reviewer"].as_str().unwrap_or_default().to_owned(),
                r["verdict"].as_str().unwrap_or_default().to_owned(),
            )
        })
        .collect()
}

fn instructions(card: &Value) -> Vec<String> {
    card["comments"]
        .as_array()
        .expect("comments")
        .iter()
        .filter(|c| c["kind"] == "instruction")
        .map(|c| c.to_string())
        .collect()
}

#[tokio::test]
async fn the_loop_returns_the_card_until_the_round_limit_and_then_waits_for_the_person() {
    let rig = Rig::new(2).await;
    assert_eq!(rig.card().await["ticket"]["status"], "running");

    // 1. The check fails (ok.txt is not committed): the card goes back to the worker at once, with
    // the output as an instruction and a round, and nobody was asked.
    rig.finish("other.txt").await;
    let card = rig
        .settled("the failing check to return the card", |c| {
            c["ticket"]["status"] == "running" && !rounds(c).is_empty()
        })
        .await;
    assert_eq!(rounds(&card), [("system".into(), "request_changes".into())]);
    let sent = instructions(&card);
    assert_eq!(sent.len(), 1, "{sent:?}");
    assert!(sent[0].contains("did not match any file"), "{}", sent[0]);
    assert!(sent[0].contains(CHECK), "{}", sent[0]);
    assert!(sent[0].contains("round 1 of 2"), "{}", sent[0]);
    assert_eq!(card["review"]["used_rounds"], 1);

    // 2. The worker fixes it: the check passes and the reviewer is next (both mode).
    rig.finish("ok.txt").await;
    let card = rig
        .settled("the reviewer's turn", |c| {
            c["review"]["awaiting"] == json!({ "by": "agent", "decides": false })
        })
        .await;
    assert_eq!(
        rounds(&card),
        [
            ("system".into(), "request_changes".into()),
            ("system".into(), "approve".into())
        ]
    );

    // 3. The verdict must name each acceptance item; one left out is refused.
    let verdict = |items: Value| {
        json!({ "event": "review_verdict", "verdict": "request_changes",
                "comment": "tidy it up", "items": items })
    };
    let (status, body) = rig
        .send(
            Method::POST,
            "/v1/tickets/DEMO-1/events",
            verdict(json!([{ "item": 1, "outcome": "pass", "note": "works" }])),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert!(
        body.to_string().contains("judge every acceptance item"),
        "{body}"
    );

    // 4. Judged item by item, request_changes returns the card to the worker: a round.
    let both = json!([
        { "item": 1, "outcome": "pass", "basis": "ran", "note": "works" },
        { "item": 2, "outcome": "fail", "note": "messy" }
    ]);
    rig.post("/v1/tickets/DEMO-1/events", verdict(both.clone()))
        .await;
    let card = rig
        .settled("the reviewer's return", |c| {
            c["ticket"]["status"] == "running"
        })
        .await;
    let comment = card["review"]["rounds"][2]["comment"]
        .as_str()
        .expect("comment");
    assert!(
        comment.starts_with("PASS 1. It logs in - works [ran]\nFAIL 2. It is tidy - messy [read]"),
        "{comment}"
    );
    assert_eq!(card["review"]["used_rounds"], 2);

    // 5. The limit is 2: the next request for changes is a round, and the card waits for the person.
    rig.finish("more.txt").await;
    rig.settled("the reviewer again", |c| {
        c["review"]["awaiting"] == json!({ "by": "agent", "decides": false })
    })
    .await;
    rig.post("/v1/tickets/DEMO-1/events", verdict(both)).await;
    let card = rig
        .settled("the card to wait for the person", |c| {
            c["review"]["awaiting"] == json!({ "by": "person" })
        })
        .await;
    assert_eq!(card["ticket"]["status"], "review");
    assert_eq!(rounds(&card).len(), 5, "the whole history: {card}");
    assert_eq!(
        card["allowed_actions"],
        json!(["accept", "return", "cancel"]),
        "now it is for the person"
    );
}

#[tokio::test]
async fn a_card_that_passes_the_check_and_the_reviewer_reaches_the_person_with_the_rounds() {
    let rig = Rig::new(3).await;
    rig.finish("ok.txt").await;
    rig.settled("the reviewer's turn", |c| {
        c["review"]["awaiting"] == json!({ "by": "agent", "decides": false })
    })
    .await;
    rig.post(
        "/v1/tickets/DEMO-1/events",
        json!({ "event": "review_verdict", "verdict": "approve", "items": [
            { "item": 1, "outcome": "pass", "note": "works" },
            { "item": 2, "outcome": "pass", "note": "tidy" } ] }),
    )
    .await;
    let card = rig.card().await;
    assert_eq!(card["review"]["awaiting"], json!({ "by": "person" }));
    assert_eq!(
        rounds(&card),
        [
            ("system".into(), "approve".into()),
            ("agent".into(), "approve".into())
        ]
    );
}

#[tokio::test]
async fn the_reviewer_started_after_a_passed_check_is_given_that_check_in_its_kickoff() {
    let rig = Rig::with_fake_reviewer().await;
    rig.finish("ok.txt").await;
    let until = Instant::now() + Duration::from_secs(30);
    let out = loop {
        let out = rig.reviewer_output();
        if out.contains("```") && out.matches("```").count() >= 2 {
            break out;
        }
        assert!(
            Instant::now() < until,
            "the reviewer got no kickoff: {out:?}"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    };
    rig.stop_reviewer();
    for want in [
        "Review ticket DEMO-1 now",
        "Toknila ran the gate",
        CHECK,
        "with PASS",
        "exited with code 0",
        "do not run the whole gate again",
    ] {
        assert!(out.contains(want), "{want:?} in the kickoff: {out}");
    }
    assert!(!out.contains("did not run the gate"), "{out}");
}

#[tokio::test]
async fn a_hand_in_on_a_ship_behind_the_base_gets_the_base_merged_in_before_anyone_reviews() {
    let rig = Rig::new(3).await;
    commit_file(&rig.repo, "from-base.txt", "base\n");
    rig.finish("ok.txt").await;
    let card = rig
        .settled("the reviewer's turn", |c| {
            c["review"]["awaiting"] == json!({ "by": "agent", "decides": false })
        })
        .await;
    assert_eq!(
        rounds(&card),
        [("system".into(), "approve".into())],
        "the check passed on the merged ship"
    );
    assert!(rig.ship.join("from-base.txt").is_file(), "base merged in");
    let out = Command::new("git")
        .arg("-C")
        .arg(&rig.ship)
        .args(["log", "-1", "--format=%p"])
        .output()
        .expect("git");
    let parents = String::from_utf8_lossy(&out.stdout);
    assert_eq!(
        parents.split_whitespace().count(),
        2,
        "a merge commit, no rebase"
    );
}

#[tokio::test]
async fn a_project_without_a_check_command_still_syncs_a_ship_behind_the_base() {
    let rig = Rig::with_check(3, None).await;
    commit_file(&rig.repo, "from-base.txt", "base\n");
    rig.finish("ok.txt").await;
    let card = rig
        .settled("the reviewer's turn", |c| {
            c["review"]["awaiting"] == json!({ "by": "agent", "decides": false })
        })
        .await;
    assert_eq!(
        rounds(&card),
        [("system".into(), "approve".into())],
        "nothing to run counts as passed"
    );
    assert!(rig.ship.join("from-base.txt").is_file(), "base merged in");
    let out = Command::new("git")
        .arg("-C")
        .arg(&rig.ship)
        .args(["log", "-1", "--format=%p"])
        .output()
        .expect("git");
    assert_eq!(
        String::from_utf8_lossy(&out.stdout)
            .split_whitespace()
            .count(),
        2,
        "a merge commit, no rebase"
    );
}

#[tokio::test]
async fn a_conflict_with_the_base_returns_the_card_to_the_worker_with_the_files_named() {
    let rig = Rig::new(3).await;
    commit_file(&rig.repo, "README.md", "base edit\n");
    commit_file(&rig.ship, "README.md", "ship edit\n");
    rig.finish("ok.txt").await;
    let card = rig
        .settled("the conflict to return the card", |c| {
            c["ticket"]["status"] == "running" && !rounds(c).is_empty()
        })
        .await;
    let sent = instructions(&card);
    assert_eq!(sent.len(), 1, "{sent:?}");
    assert!(
        sent[0].contains("Conflicting files: README.md"),
        "{}",
        sent[0]
    );
    assert!(sent[0].contains("git merge develop"), "{}", sent[0]);
    assert_eq!(card["review"]["awaiting"], Value::Null, "nobody is asked");
}
