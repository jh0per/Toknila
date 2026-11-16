//! Accept checks again when main moved, and the extra checks of a project, through the full
//! router with a real git ship and the real shell checker. The check command adds one entry to
//! the repository's git config (`toknila.checks`, shared by every worktree; no shell redirect, so
//! the same words work in `sh` and in `cmd.exe`), then needs `ok.txt` to be tracked: so a run is
//! counted and a main that removed `ok.txt` fails the check. The test plays the agent's git and its
//! `finished`; the agent process is the workspace's `fake-agent`.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tokio::sync::Notify;
use toknila_app::runner::Runner;
use toknila_core::AgentRole;
use toknila_daemon::server::{AppState, router};
use toknila_daemon::shipyard::GitShipyard;
use toknila_daemon::store::Store;
use tower::ServiceExt;

const TOKEN: &str = "t";

fn git(dir: &Path, args: &[&str]) -> String {
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
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

fn commit_file(dir: &Path, name: &str, content: &str) {
    if let Some(parent) = dir.join(name).parent() {
        std::fs::create_dir_all(parent).expect("mkdir");
    }
    std::fs::write(dir.join(name), content).expect("write");
    git(dir, &["add", name]);
    git(dir, &["commit", "--quiet", "-m", name]);
}

/// How many times the command that adds `toknila.<key>` to the repository's config has run.
fn runs(repo: &Path, key: &str) -> usize {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["config", "--get-all", &format!("toknila.{key}")])
        .output()
        .expect("git runs");
    String::from_utf8_lossy(&out.stdout).lines().count()
}

struct Rig {
    _tmp: tempfile::TempDir,
    app: axum::Router,
    store: Store,
    runners: Arc<toknila_daemon::runner::Runners>,
    _ended: tokio::sync::mpsc::UnboundedReceiver<toknila_app::runner::RunEnded>,
    repo: PathBuf,
    ship: PathBuf,
    ticket: toknila_core::TicketId,
}

impl Rig {
    async fn new() -> Rig {
        // The same words on every platform's shell: count the run, then require `ok.txt`.
        Rig::with_command(
            "git config --add toknila.checks 1 && git ls-files --error-unmatch ok.txt",
        )
        .await
    }

    async fn with_command(command: &str) -> Rig {
        let tmp = tempfile::tempdir().expect("tmp");
        let repo = tmp.path().join("repo");
        std::fs::create_dir_all(&repo).expect("mkdir");
        git(&repo, &["init", "--quiet"]);
        git(&repo, &["symbolic-ref", "HEAD", "refs/heads/develop"]);
        commit_file(&repo, "README.md", "hi\n");
        commit_file(&repo, "ok.txt", "ok\n");

        let (runners, ended) =
            toknila_daemon::runner::Runners::new(tmp.path().join("runs"), tmp.path().join("p"));
        std::fs::create_dir_all(tmp.path().join("p")).expect("profiles");
        let agent = env!("CARGO_BIN_EXE_fake-agent");
        std::fs::write(
            tmp.path().join("p").join("fake.toml"),
            format!("command = ['{agent}', 'print', '{{kickoff}}', 'hold']\n"),
        )
        .expect("profile");
        let store = Store::open(&tmp.path().join("t.db"))
            .expect("store")
            .with_shipyard(Arc::new(GitShipyard::new(tmp.path().join("ships"))))
            .with_merger(Arc::new(toknila_daemon::github::Mergers::new()))
            .with_runner(runners.clone());
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
            runners,
            _ended: ended,
            repo: repo.clone(),
            ship: PathBuf::new(),
            ticket: toknila_core::TicketId::new(),
        };
        rig.call(
            Method::POST,
            "/v1/projects",
            json!({ "key": "DEMO", "repo_path": repo.to_string_lossy(),
                    "default_profile": "fake", "default_review_mode": "human",
                    "check_command": command }),
        )
        .await;
        rig.call(
            Method::PATCH,
            "/v1/projects/DEMO",
            json!({ "extra_checks": [
                { "paths": ["apps/web/**"], "command": "git config --add toknila.extras 1" }
            ] }),
        )
        .await;
        rig.call(
            Method::POST,
            "/v1/projects/DEMO/chains",
            json!({ "name": "auth" }),
        )
        .await;
        let card = rig
            .call(
                Method::POST,
                "/v1/chains/auth/tickets",
                json!({ "title": "Edit readme", "prompt": "Say hi." }),
            )
            .await;
        rig.ticket = card["ticket"]["id"]
            .as_str()
            .and_then(|id| id.parse().ok())
            .expect("ticket id");
        rig.output_with("DEMO-1").await;
        rig.ship = PathBuf::from(
            rig.card().await["ship"]["path"]
                .as_str()
                .expect("the card has a ship"),
        );
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
        // The effects of the command (git, the check, the merge) run after the answer.
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

    async fn card(&self) -> Value {
        self.call(Method::GET, "/v1/tickets/DEMO-1", Value::Null)
            .await
    }

    fn output(&self) -> String {
        self.runners
            .read(self.ticket, AgentRole::Worker, None)
            .map(|c| String::from_utf8_lossy(&c.bytes).into_owned())
            .unwrap_or_default()
            .replace(['\r', '\n'], "")
    }

    async fn output_with(&self, what: &str) -> String {
        let until = Instant::now() + Duration::from_secs(30);
        loop {
            let out = self.output();
            if out.contains(what) {
                return out;
            }
            assert!(
                Instant::now() < until,
                "the agent never printed {what:?}: {out:?}"
            );
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }

    async fn hand_in(&self, summary: &str) {
        self.call(
            Method::POST,
            "/v1/tickets/DEMO-1/events",
            json!({ "event": "finished", "summary": toknila_core::handin::from_plain(summary) }),
        )
        .await;
    }

    async fn accept(&self) {
        self.call(Method::POST, "/v1/tickets/DEMO-1/actions/accept", json!({}))
            .await;
    }

    /// The agent's work is in and handed in; its process is stopped (as at a real hand-in).
    async fn handed_in_with(&self, file: &str) {
        commit_file(&self.ship, file, "work\n");
        self.hand_in("done").await;
        self.runners.stop_role(self.ticket, AgentRole::Worker);
    }

    fn develop(&self) -> String {
        git(&self.repo, &["rev-parse", "develop"])
    }

    /// The text of every round of the card.
    async fn rounds(&self) -> Vec<String> {
        self.card().await["ticket"]["review"]["rounds"]
            .as_array()
            .map(|all| {
                all.iter()
                    .filter_map(|r| r["comment"].as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default()
    }
}

#[tokio::test]
async fn the_check_records_the_main_commit_and_the_ship_head_it_ran_on() {
    let rig = Rig::new().await;
    let main = rig.develop();
    rig.handed_in_with("feature.txt").await;
    let card = rig.card().await;
    let kept = &card["ticket"]["review"]["last_check"];
    assert_eq!(kept["passed"], true, "{card}");
    assert_eq!(kept["base"].as_str(), Some(main.as_str()), "{kept}");
    assert_eq!(
        kept["head"].as_str(),
        Some(git(&rig.ship, &["rev-parse", "HEAD"]).as_str()),
        "{kept}"
    );
    assert_eq!(runs(&rig.repo, "checks"), 1);
}

#[tokio::test]
async fn accept_after_main_moved_runs_the_check_once_and_merges_with_no_further_action() {
    let rig = Rig::new().await;
    rig.handed_in_with("feature.txt").await;
    assert_eq!(runs(&rig.repo, "checks"), 1);

    // Main takes a commit of another card; then the person accepts, once.
    commit_file(&rig.repo, "other.txt", "other\n");
    let moved_to = rig.develop();
    rig.accept().await;

    let card = rig.card().await;
    assert_eq!(card["ticket"]["status"], "merged", "{card}");
    assert_eq!(
        runs(&rig.repo, "checks"),
        2,
        "the check ran once more, not more"
    );
    let rounds = rig.rounds().await;
    assert!(
        rounds
            .iter()
            .any(|r| r == "main moved since the check: 1 commit"),
        "{rounds:?}"
    );
    // The re-check saw the new main; and both files are on develop.
    assert_eq!(
        card["ticket"]["review"]["last_check"]["base"].as_str(),
        Some(moved_to.as_str())
    );
    assert_eq!(git(&rig.repo, &["show", "develop:feature.txt"]), "work");
    assert_eq!(git(&rig.repo, &["show", "develop:other.txt"]), "other");
}

#[tokio::test]
async fn accept_with_main_where_the_check_saw_it_merges_at_once() {
    let rig = Rig::new().await;
    rig.handed_in_with("feature.txt").await;
    rig.accept().await;
    assert_eq!(rig.card().await["ticket"]["status"], "merged");
    assert_eq!(runs(&rig.repo, "checks"), 1, "the check is not run again");
}

#[tokio::test]
async fn a_failing_check_after_main_moved_returns_the_card_to_the_agent_as_a_round() {
    let rig = Rig::new().await;
    rig.handed_in_with("feature.txt").await;
    let before = rig.develop();

    // Main removes `ok.txt`, which the check requires.
    git(&rig.repo, &["rm", "--quiet", "ok.txt"]);
    git(&rig.repo, &["commit", "--quiet", "-m", "drop ok.txt"]);
    let main = rig.develop();
    rig.accept().await;

    let card = rig.card().await;
    assert_eq!(card["ticket"]["status"], "running", "{card}");
    // The first check passed once; the one Accept asked for failed, and failed again on its
    // second run: that is a round, not a flaky pass.
    assert_eq!(runs(&rig.repo, "checks"), 3);
    assert_eq!(rig.develop(), main, "nothing was merged");
    assert_ne!(before, main);
    let rounds = rig.rounds().await;
    assert!(
        rounds
            .iter()
            .any(|r| r == "main moved since the check: 1 commit"),
        "{rounds:?}"
    );
    let last = rounds.last().expect("a round");
    assert!(
        last.contains("ok.txt"),
        "the check's output is the round: {last}"
    );
    rig.runners.stop_role(rig.ticket, AgentRole::Worker);
}

#[tokio::test]
async fn an_extra_check_runs_only_when_a_matching_file_changed_and_its_line_is_in_the_round() {
    let rig = Rig::new().await;
    rig.handed_in_with("feature.txt").await;
    assert_eq!(
        runs(&rig.repo, "extras"),
        0,
        "no file under apps/web changed"
    );
    let rounds = rig.rounds().await;
    assert_eq!(rounds.last().map(String::as_str), Some("The check passed."));

    // Hand in again with a web file: the extra check runs after the command.
    let other = Rig::new().await;
    other.handed_in_with("apps/web/a.txt").await;
    assert_eq!(runs(&other.repo, "extras"), 1);
    assert_eq!(runs(&other.repo, "checks"), 1);
    let rounds = other.rounds().await;
    let last = rounds.last().expect("a round");
    assert!(
        last.starts_with(
            "The check passed.\nextra check apps/web/**: PASS git config --add toknila.extras 1"
        ),
        "{last}"
    );
    assert!(last.contains(", 0 s"), "{last}");
}

/// Fails the first time and passes the second, with the repository's git config (shared by
/// every worktree) as the counter; `&&` and `||` mean the same in `sh` and in `cmd.exe`, and
/// there is no redirect. The test seeds `toknila.n` with two values. Run 1: setting `n` to one
/// value is refused (error: cannot overwrite multiple values), the `||` branch removes them, and
/// the last step finds nothing to unset and fails. Run 2: `n` is absent, so setting it works, the
/// `||` branch is skipped, and the last step unsets it and passes.
const FAILS_ONCE: &str = "git config --add toknila.checks 1 && git config toknila.n y || git config --unset-all toknila.n && git config --unset toknila.n";

fn seed_two_values(repo: &Path) {
    git(repo, &["config", "--add", "toknila.n", "a"]);
    git(repo, &["config", "--add", "toknila.n", "b"]);
}

async fn inbox_items(rig: &Rig, kind: &str) -> Vec<Value> {
    let all = rig
        .call(Method::GET, "/v1/projects/DEMO/inbox", Value::Null)
        .await;
    all.as_array()
        .expect("a list")
        .iter()
        .filter(|i| i["kind"] == kind)
        .cloned()
        .collect()
}

#[tokio::test]
async fn a_check_that_fails_once_and_passes_again_is_a_flaky_pass_not_a_round() {
    let rig = Rig::with_command(FAILS_ONCE).await;
    seed_two_values(&rig.repo);
    rig.handed_in_with("feature.txt").await;
    let card = rig.card().await;
    let kept = &card["ticket"]["review"]["last_check"];
    assert_eq!(kept["passed"], true, "{card}");
    assert_eq!(kept["flaky"], true, "{kept}");
    assert!(
        kept["flaky_output"]
            .as_str()
            .is_some_and(|o| o.contains("cannot overwrite multiple values")),
        "{kept}"
    );
    assert_eq!(card["ticket"]["status"], "review", "{card}");
    let rounds = rig.rounds().await;
    assert_eq!(rounds.len(), 1, "one round, a pass: {rounds:?}");
    assert!(
        rounds[0].starts_with("The check passed on the second run; the first failed:\n"),
        "{rounds:?}"
    );
    assert!(
        rounds[0].contains("cannot overwrite multiple values"),
        "{rounds:?}"
    );
    assert_eq!(
        runs(&rig.repo, "checks"),
        2,
        "run twice, on the same commit"
    );

    // The person learns of it, and may dismiss it.
    let items = inbox_items(&rig, "flaky").await;
    assert_eq!(items.len(), 1, "{items:?}");
    assert_eq!(
        items[0]["text"],
        "A test failed once and passed on the second run while checking DEMO-1: error: cannot overwrite multiple values with a single value"
    );
    assert!(
        items[0]["detail"]
            .as_str()
            .is_some_and(|d| d.contains("cannot overwrite multiple values")),
        "{:?}",
        items[0]
    );
    rig.call(
        Method::POST,
        "/v1/tickets/DEMO-1/flaky/dismiss",
        Value::Null,
    )
    .await;
    assert!(inbox_items(&rig, "flaky").await.is_empty());
    let (status, _) = rig
        .send(
            Method::POST,
            "/v1/tickets/DEMO-1/flaky/dismiss",
            Value::Null,
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "nothing left to dismiss");
}

#[tokio::test]
async fn a_check_that_fails_twice_is_a_round_with_the_second_output() {
    let rig = Rig::with_command(
        "git config --add toknila.checks 1 && echo test einval FAILED && git config --get toknila.nope",
    )
    .await;
    rig.handed_in_with("feature.txt").await;
    let card = rig.card().await;
    assert_eq!(card["ticket"]["status"], "running", "{card}");
    assert_eq!(runs(&rig.repo, "checks"), 2, "one more run, no more");
    assert_ne!(card["ticket"]["review"]["last_check"]["flaky"], true);
    let last = rig.rounds().await.pop().expect("a round");
    assert!(last.contains("test einval FAILED"), "{last}");
    assert!(inbox_items(&rig, "flaky").await.is_empty());
    rig.runners.stop_role(rig.ticket, AgentRole::Worker);
}

#[tokio::test]
async fn a_command_that_cannot_run_is_not_run_again() {
    // Exit 127 in `sh`, 9009 and a message in `cmd.exe`; the counter runs only with the command.
    let rig =
        Rig::with_command("git config --add toknila.checks 1 && toknila-no-such-program-zz").await;
    rig.handed_in_with("feature.txt").await;
    assert_eq!(runs(&rig.repo, "checks"), 1, "a missing program runs once");
    assert!(inbox_items(&rig, "flaky").await.is_empty());
}
