//! A card that waits for a person frees its slot and comes back when the answer arrives, through
//! the full router with real runner processes (the workspace's `fake-agent`, never a real agent):
//! after the grace the agent's process is gone and the next ready card starts; a comment, a thread
//! approval or a permission answer starts the card again with the profile's resume argument and
//! the answer as the run's first input, and a profile without a resume form starts fresh.

use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::http::{Method, Request};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tokio::sync::Notify;
use toknila_core::{AgentRole, TicketId};
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

/// What the first run of a card does before it waits for a person.
#[derive(Clone, Copy)]
enum Wait {
    /// Asks a question (`ask_human`).
    Question,
    /// Shows a permission prompt.
    Permission,
}

struct Rig {
    _tmp: tempfile::TempDir,
    app: axum::Router,
    store: Store,
    runners: Arc<Runners>,
}

impl Rig {
    /// A project with `slots` slots whose cards run `asker` (waits as `wait` says, then holds; a
    /// resumed run records how it started and holds) or `holder` (just holds). `resume` gives the
    /// asker a resume form.
    async fn new(slots: u32, wait: Wait, resume: bool, profiles: &[&str]) -> Rig {
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
        let activity =
            r#"{"signal":"hook:post_tool_use","session_id":"sess-1","transcript_path":"/x.jsonl"}"#;
        let (path, body) = match wait {
            Wait::Question => (
                "/tickets/{ticket}/events",
                r#"{"event":"ask_human","text":"which db?"}"#,
            ),
            Wait::Permission => (
                "/tickets/{ticket}/activity",
                r#"{"signal":"hook:notification","prompt":"Claude needs your permission to use Bash"}"#,
            ),
        };
        let form = if resume {
            "resume = ['--resume', '{session}']\n"
        } else {
            ""
        };
        std::fs::write(
            dir.join("asker.toml"),
            format!(
                "command = ['{AGENT}', '--prompt', '{{kickoff}}', 'post', '/tickets/{{ticket}}/activity', '{activity}', \
                 'ifresumed', 'print', 'asking', 'post', '{path}', '{body}', 'hold']\n{form}"
            ),
        )
        .expect("profile");
        // The asker whose agent does not know the session id it is resumed with.
        std::fs::write(
            dir.join("refuser.toml"),
            format!(
                "command = ['{AGENT}', '--prompt', '{{kickoff}}', 'refuseresume', 'post', \
                 '/tickets/{{ticket}}/activity', '{activity}', 'print', 'asking', 'post', '{path}', \
                 '{body}', 'hold']\n{form}"
            ),
        )
        .expect("profile");
        std::fs::write(
            dir.join("holder.toml"),
            format!("command = ['{AGENT}', 'print', 'holding', 'hold']\n"),
        )
        .expect("profile");
        let (runners, ended) = Runners::new(tmp.path().join("runs"), dir.clone());
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
        // The agents report to a real listener, as hooks do.
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
            json!({ "key": "DEMO", "repo_path": repo.to_string_lossy(), "max_parallel": slots,
                    "default_profile": "asker" }),
        )
        .await;
        rig.send(
            Method::POST,
            "/v1/projects/DEMO/chains",
            json!({ "name": "auth" }),
        )
        .await;
        for (n, profile) in profiles.iter().enumerate() {
            rig.send(
                Method::POST,
                "/v1/chains/auth/tickets",
                json!({ "title": format!("Card {}", n + 1), "prompt": "Do it.", "profile": profile }),
            )
            .await;
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
        let status = resp.status();
        let bytes = resp.into_body().collect().await.expect("body").to_bytes();
        let v: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        assert!(
            status.is_success(),
            "{method_uri}: {status} {v}",
            method_uri = uri
        );
        v
    }

    async fn card(&self, n: usize) -> Value {
        self.send(Method::GET, &format!("/v1/tickets/DEMO-{n}"), Value::Null)
            .await
    }

    async fn id(&self, n: usize) -> TicketId {
        self.card(n).await["ticket"]["id"]
            .as_str()
            .expect("id")
            .parse()
            .expect("ticket id")
    }

    /// Polls until `ok` holds of the card, running the pause check of the silence ticker each
    /// round, and returns the card.
    async fn until(&self, what: &str, n: usize, ok: impl Fn(&Value) -> bool) -> Value {
        let until = Instant::now() + WAIT;
        loop {
            silence_tick(&self.store, &self.runners, now()).await;
            let c = self.card(n).await;
            if ok(&c) {
                return c;
            }
            assert!(
                Instant::now() < until,
                "card {n}: no {what}: {}\n{}",
                c["ticket"],
                self.output(n).await
            );
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    /// The newest worker run's output of card `n`.
    async fn output(&self, n: usize) -> String {
        let id = self.id(n).await;
        self.runners
            .read(id, AgentRole::Worker, None)
            .map(|c| String::from_utf8_lossy(&c.bytes).into_owned())
            .unwrap_or_default()
    }

    async fn output_has(&self, n: usize, what: &str) -> String {
        let until = Instant::now() + WAIT;
        loop {
            let out = self.output(n).await;
            if out.contains(what) {
                return out;
            }
            assert!(Instant::now() < until, "no {what:?} in {out:?}");
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    /// Card `n` has asked and its agent is paused: no process, a session kept.
    async fn paused(&self, n: usize) -> Value {
        let c = self
            .until("a paused agent", n, |c| {
                !c["ticket"]["agent_paused"].is_null()
            })
            .await;
        let id = self.id(n).await;
        assert!(!self.runners.live().contains(&id), "the process is gone");
        assert_eq!(c["ticket"]["status"], "waiting");
        assert_eq!(c["ticket"]["agent_paused"]["session"], "sess-1");
        c
    }
}

fn is(status: &'static str) -> impl Fn(&Value) -> bool {
    move |c| c["ticket"]["status"] == status && c["ticket"]["agent_paused"].is_null()
}

#[tokio::test]
async fn a_waiting_card_frees_its_slot_and_a_comment_resumes_it_when_one_is_free() {
    let rig = Rig::new(1, Wait::Question, true, &["asker", "holder"]).await;
    // One slot: the asker holds it while it waits with its agent alive, and the second card
    // queues behind it.
    rig.output_has(1, "asking").await;
    assert_eq!(rig.card(2).await["queue_position"], 1);
    // After the grace the agent's process is gone, the log says why, and the next card starts.
    let paused = rig.paused(1).await;
    rig.until("the second card running", 2, is("running")).await;
    assert!(
        rig.output(1)
            .await
            .contains("paused: waiting for your answer to: which db?")
    );
    assert_eq!(paused["queue_position"], Value::Null);
    // The Agents list shows the card that works, not the one that waits with no process.
    rig.output_has(2, "holding").await;
    let agents = rig
        .send(Method::GET, "/v1/agents", Value::Null)
        .await
        .to_string();
    assert!(
        agents.contains("DEMO-2") && !agents.contains("DEMO-1"),
        "{agents}"
    );
    // The answer arrives while the slot is taken: the card queues like a ready one.
    let answered = rig
        .send(
            Method::POST,
            "/v1/tickets/DEMO-1/comments",
            json!({ "text": "use sqlite" }),
        )
        .await;
    assert_eq!(answered["ticket"]["agent_paused"]["resume"], true);
    assert_eq!(answered["queue_position"], 1);
    // A slot frees: the card starts again from its session with the answer as its first input.
    rig.send(
        Method::POST,
        "/v1/tickets/DEMO-2/actions/cancel",
        json!({ "text": "not needed" }),
    )
    .await;
    rig.until("the first card running", 1, is("running")).await;
    let out = rig.output_has(1, "resume=sess-1").await;
    assert!(out.contains("resume=sess-1"), "{out}");
    assert!(
        out.contains("prompt=") && out.contains("use sqlite"),
        "{out}"
    );
    assert!(
        !out.contains("asking"),
        "a resumed run does not ask again: {out}"
    );
}

#[tokio::test]
async fn a_comment_restarts_a_paused_card_at_once_when_a_slot_is_free() {
    let rig = Rig::new(2, Wait::Question, true, &["asker"]).await;
    rig.paused(1).await;
    rig.send(
        Method::POST,
        "/v1/tickets/DEMO-1/comments",
        json!({ "text": "use sqlite" }),
    )
    .await;
    rig.until("running again", 1, is("running")).await;
    let out = rig.output_has(1, "resume=sess-1").await;
    assert!(out.contains("use sqlite"), "{out}");
}

#[tokio::test]
async fn a_thread_approval_restarts_a_paused_card_with_the_decision_as_its_first_input() {
    let rig = Rig::new(2, Wait::Question, true, &["asker"]).await;
    rig.paused(1).await;
    rig.send(
        Method::POST,
        "/v1/tickets/DEMO-1/threads",
        json!({ "kind": "design", "title": "The schema" }),
    )
    .await;
    rig.send(
        Method::POST,
        "/v1/tickets/DEMO-1/threads/1/versions",
        json!({ "format": "markdown", "body": "# schema" }),
    )
    .await;
    rig.send(
        Method::POST,
        "/v1/tickets/DEMO-1/threads/1/approve",
        Value::Null,
    )
    .await;
    rig.until("running again", 1, is("running")).await;
    let out = rig.output_has(1, "resume=sess-1").await;
    assert!(
        out.contains("The schema") && out.contains("is approved"),
        "{out}"
    );
}

#[tokio::test]
async fn a_permission_answer_restarts_a_paused_card_with_the_answer_as_its_first_input() {
    let rig = Rig::new(2, Wait::Permission, true, &["asker"]).await;
    let c = rig.paused(1).await;
    assert!(c["ticket"]["status_reason"]["kind"] == "permission", "{c}");
    rig.send(
        Method::POST,
        "/v1/tickets/DEMO-1/runner/answer",
        json!({ "answer": "allow" }),
    )
    .await;
    rig.until("running again", 1, is("running")).await;
    let out = rig.output_has(1, "resume=sess-1").await;
    assert!(out.contains("Allowed"), "{out}");
}

#[tokio::test]
async fn a_design_pin_answer_restarts_a_paused_card_with_the_answer_as_its_first_input() {
    use toknila_app::pins::{self, NewPin};
    use toknila_core::design::Region;
    use toknila_core::{Actor, DesignId, ProjectId};
    use toknila_daemon::http::error::ApiError;

    let rig = Rig::new(2, Wait::Question, true, &["asker"]).await;
    rig.paused(1).await;
    let project: ProjectId = rig
        .send(Method::GET, "/v1/projects/DEMO", Value::Null)
        .await["id"]
        .as_str()
        .expect("project id")
        .parse()
        .expect("id");
    let new = NewPin {
        design: DesignId::new(),
        design_name: "Home".into(),
        region: Region {
            x: 10,
            y: 10,
            w: 20,
            h: 20,
        },
        text: "Which colour?".into(),
        card: Some("DEMO-1".into()),
    };
    let (pin, _) = rig
        .store
        .write(Actor::Agent, now(), move |tx| -> Result<_, ApiError> {
            Ok(pins::ask(tx, project, new)?)
        })
        .await
        .expect("pin");
    rig.send(
        Method::POST,
        &format!("/v1/projects/DEMO/design-pins/{}/answer", pin.id),
        json!({ "text": "teal" }),
    )
    .await;
    rig.until("running again", 1, is("running")).await;
    let out = rig.output_has(1, "resume=sess-1").await;
    assert!(out.contains("teal"), "{out}");
}

#[tokio::test]
async fn a_refused_resume_falls_back_to_one_fresh_start_with_the_kickoff_and_the_answer() {
    let rig = Rig::new(2, Wait::Question, true, &["refuser"]).await;
    rig.paused(1).await;
    rig.send(
        Method::POST,
        "/v1/tickets/DEMO-1/comments",
        json!({ "text": "use sqlite" }),
    )
    .await;
    // The resumed run printed the refusal and ended; the card is not blocked as crashed: its agent
    // starts again from the kickoff, which carries the answer.
    let until = Instant::now() + WAIT;
    let out = loop {
        let out = rig.output(1).await;
        if out.contains("prompt=Start ticket DEMO-1")
            && out.contains("use sqlite")
            && !out.contains("resume=")
        {
            break out;
        }
        assert!(Instant::now() < until, "no fresh start: {out:?}");
        tokio::time::sleep(Duration::from_millis(50)).await;
    };
    assert!(!out.contains("No conversation found"), "{out}");
    // The fresh run asks again and its card waits; it never reads Blocked.
    let c = rig
        .until("waiting again", 1, |c| c["ticket"]["status"] == "waiting")
        .await;
    assert_eq!(c["ticket"]["status"], "waiting", "{c}");
}

#[tokio::test]
async fn a_profile_without_a_resume_form_starts_fresh_with_the_kickoff_and_the_answer() {
    let rig = Rig::new(2, Wait::Question, false, &["asker"]).await;
    rig.paused(1).await;
    rig.send(
        Method::POST,
        "/v1/tickets/DEMO-1/comments",
        json!({ "text": "use sqlite" }),
    )
    .await;
    // The new run's first prompt is the kickoff, and it carries the answer that waited.
    let out = rig.output_has(1, "use sqlite").await;
    assert!(!out.contains("resume="), "{out}");
    assert!(out.contains("prompt=Start ticket DEMO-1"), "{out}");
}
