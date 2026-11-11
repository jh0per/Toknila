//! What an agent asks and a person answers, through the full router: a permission hook that
//! waits for the decision (worker and reviewer), what the question says, the startup wait, and
//! the journal.

use std::time::Duration;

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};
use toknila_core::{AgentRole, Timestamp};

use super::Api;
use crate::server::now;

const INBOX: &str = "/v1/projects/DEMO/inbox";

async fn setup() -> (Api, String) {
    let api = Api::new();
    api.demo_with(&["A"]).await;
    let (token, _) = api.agent_token("DEMO-1").await;
    api.call_with(
        &token,
        Method::POST,
        "/v1/tickets/DEMO-1/events",
        Some(json!({ "event": "started" })),
    )
    .await;
    (api, token)
}

async fn questions(api: &Api) -> Vec<Value> {
    api.get(INBOX)
        .await
        .as_array()
        .expect("items")
        .iter()
        .filter(|i| i["kind"] == "question")
        .cloned()
        .collect()
}

/// Waits until the inbox lists `n` questions.
async fn wait_for(api: &Api, n: usize) -> Vec<Value> {
    for _ in 0..200 {
        let q = questions(api).await;
        if q.len() == n {
            return q;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!(
        "the inbox never listed {n} questions: {:?}",
        questions(api).await
    );
}

async fn permission(api: &Api, token: &str, tool: &str) -> (StatusCode, Value) {
    api.call_with(
        token,
        Method::POST,
        "/v1/tickets/DEMO-1/permission",
        Some(json!({ "tool": tool })),
    )
    .await
}

async fn answer(api: &Api, body: Value) -> (StatusCode, Value) {
    api.call(Method::POST, "/v1/tickets/DEMO-1/runner/answer", Some(body))
        .await
}

async fn journal(api: &Api) -> Vec<Value> {
    let events = api.get("/v1/events?since=0&limit=10000").await;
    events
        .as_array()
        .expect("events")
        .iter()
        .filter(|e| {
            e["kind"]
                .as_str()
                .is_some_and(|k| k.starts_with("question_"))
        })
        .cloned()
        .collect()
}

#[tokio::test]
async fn a_workers_permission_hook_waits_and_allow_resumes_the_card_at_once() {
    let (api, token) = setup().await;
    let hook = permission(&api, &token, "Bash: cargo test -p core");
    let person = async {
        let q = wait_for(&api, 1).await;
        let item = &q[0];
        assert_eq!(item["role"], "worker");
        assert_eq!(item["answers"], json!(["allow", "deny"]));
        assert_eq!(
            item["text"],
            "Claude Code asks for permission: Bash: cargo test -p core"
        );
        assert_eq!(api.status("DEMO-1").await, "waiting");
        let (s, v) = answer(&api, json!({ "answer": "allow" })).await;
        assert_eq!(s, StatusCode::NO_CONTENT, "{v}");
        // Running again at once: no tool has finished, the hook has not even replied yet.
        assert_eq!(api.status("DEMO-1").await, "running");
    };
    let ((status, reply), ()) = tokio::join!(hook, person);
    assert_eq!(status, StatusCode::OK);
    assert_eq!(reply, json!({ "decision": "allow" }));
    assert!(questions(&api).await.is_empty());
    let kinds: Vec<_> = journal(&api).await;
    let answered = kinds
        .iter()
        .find(|e| e["kind"] == "question_answered")
        .expect("journaled");
    assert_eq!(answered["actor"], "human");
    assert_eq!(answered["payload"]["role"], "worker");
    assert_eq!(answered["payload"]["answer"], "allow");
}

#[tokio::test]
async fn a_reviewers_permission_shows_in_the_inbox_and_deny_carries_the_comment() {
    let (api, _) = setup().await;
    let (reviewer, _) = api.agent_token_as("DEMO-1", AgentRole::Reviewer).await;
    let hook = permission(&api, &reviewer, "Bash: git diff main");
    let person = async {
        let q = wait_for(&api, 1).await;
        assert_eq!(q[0]["role"], "reviewer");
        assert_eq!(q[0]["answers"], json!(["allow", "deny"]));
        assert!(
            q[0]["text"]
                .as_str()
                .unwrap_or("")
                .contains("git diff main")
        );
        assert_eq!(
            api.status("DEMO-1").await,
            "running",
            "the card does not wait for a reviewer"
        );
        let (s, v) = answer(
            &api,
            json!({ "role": "reviewer", "answer": "deny", "text": "read the diff in the PR" }),
        )
        .await;
        assert_eq!(s, StatusCode::NO_CONTENT, "{v}");
    };
    let ((status, reply), ()) = tokio::join!(hook, person);
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        reply,
        json!({ "decision": "deny", "message": "read the diff in the PR" })
    );
    let events = journal(&api).await;
    let kinds: Vec<_> = events
        .iter()
        .map(|e| e["kind"].as_str().unwrap_or(""))
        .collect();
    assert_eq!(kinds, ["question_raised", "question_answered"]);
    assert_eq!(events[1]["payload"]["role"], "reviewer");
    assert_eq!(events[1]["payload"]["answer"], "deny");
}

#[tokio::test]
async fn a_workers_shell_edit_is_denied_at_once_with_the_edit_reason_and_never_reaches_the_inbox() {
    let (api, token) = setup().await;
    for command in [
        "python3 - <<'EOF'\nopen('a.rs','w').write('x')\nEOF",
        "cat > crates/core/src/lib.rs <<'EOF'\nfn x() {}\nEOF",
        "sed -i 's/a/b/' crates/core/src/lib.rs",
        "perl -pi -e 's/a/b/' crates/core/src/lib.rs",
    ] {
        let (status, reply) = permission(&api, &token, &format!("Bash: {command}")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(reply["decision"], "deny", "{command}");
        let message = reply["message"].as_str().expect("reason");
        assert!(
            message.contains("Edit")
                && message.contains("Write")
                && message.contains("without asking"),
            "{message}"
        );
        assert!(questions(&api).await.is_empty(), "{command}");
        assert_eq!(api.status("DEMO-1").await, "running", "{command}");
    }
    // A write into the scratchpad and a plain read are asked about as before.
    api.runners.set_permission_wait(Duration::from_millis(60));
    for command in [
        "cat > /tmp/claude-1/p/scratchpad/n.md <<'EOF'\nhi\nEOF",
        "sed -n '1,5p' src/lib.rs",
    ] {
        let (_, reply) = permission(&api, &token, &format!("Bash: {command}")).await;
        assert_eq!(reply, json!({ "decision": "ask" }), "{command}");
        api.call_with(
            &token,
            Method::POST,
            "/v1/tickets/DEMO-1/activity",
            Some(json!({ "signal": "hook:post_tool_use" })),
        )
        .await;
    }
}

#[tokio::test]
async fn a_hook_nobody_answers_in_time_lets_the_agent_show_its_own_dialog() {
    let (api, token) = setup().await;
    api.runners.set_permission_wait(Duration::from_millis(60));
    let (status, reply) = permission(&api, &token, "Bash: ls").await;
    assert_eq!(
        (status, reply),
        (StatusCode::OK, json!({ "decision": "ask" }))
    );
    // The card still waits (the dialog is on the agent's screen); a tool that ends resumes it.
    assert_eq!(api.status("DEMO-1").await, "waiting");
    let (s, _) = api
        .call_with(
            &token,
            Method::POST,
            "/v1/tickets/DEMO-1/activity",
            Some(json!({ "signal": "hook:post_tool_use" })),
        )
        .await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    assert_eq!(api.status("DEMO-1").await, "running");
}

#[tokio::test]
async fn the_last_tool_is_attached_to_a_permission_notification() {
    let (api, token) = setup().await;
    let hook = |body: Value| {
        api.call_with(
            &token,
            Method::POST,
            "/v1/tickets/DEMO-1/activity",
            Some(body),
        )
    };
    let (s, _) = hook(json!({ "signal": "hook:pre_tool_use", "tool": "Bash: rm -rf build" })).await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    let (s, _) = hook(
        json!({ "signal": "hook:notification", "prompt": "Claude needs your permission to use Bash" }),
    )
    .await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    let q = wait_for(&api, 1).await;
    assert_eq!(
        q[0]["text"],
        "Claude needs your permission to use Bash: Bash: rm -rf build"
    );
    assert_eq!(q[0]["answers"], json!(["allow", "deny"]));
    assert_eq!(api.status("DEMO-1").await, "waiting");
}

#[tokio::test]
async fn a_long_tool_summary_is_capped() {
    let (api, token) = setup().await;
    let long = "x".repeat(2000);
    api.call_with(
        &token,
        Method::POST,
        "/v1/tickets/DEMO-1/activity",
        Some(json!({ "signal": "hook:pre_tool_use", "tool": long })),
    )
    .await;
    api.call_with(
        &token,
        Method::POST,
        "/v1/tickets/DEMO-1/activity",
        Some(json!({ "signal": "hook:notification", "prompt": "p" })),
    )
    .await;
    let q = wait_for(&api, 1).await;
    assert!(
        q[0]["text"].as_str().unwrap_or("").chars().count() < 340,
        "{}",
        q[0]["text"]
    );
    assert_eq!(
        q[0]["detail"],
        format!("Bash: {long}").replace("Bash: ", ""),
        "the full text travels with the question"
    );
}

#[tokio::test]
async fn a_multiline_request_keeps_its_lines_for_the_worker_and_the_reviewer_and_a_short_one_has_none()
 {
    let (api, token) = setup().await;
    let script = "cargo test -p toknila-core\n  -- --nocapture";
    let tool = format!("Bash: {script}");
    let hook = permission(&api, &token, &tool);
    let person = async {
        let q = wait_for(&api, 1).await;
        assert_eq!(
            q[0]["text"],
            "Claude Code asks for permission: Bash: cargo test -p toknila-core -- --nocapture"
        );
        assert_eq!(q[0]["detail"], format!("Bash: {script}"));
        let shown = api.get("/v1/tickets/DEMO-1").await;
        assert_eq!(
            shown["ticket"]["status_reason"]["detail"],
            format!("Bash: {script}")
        );
        answer(&api, json!({ "answer": "allow" })).await;
    };
    tokio::join!(hook, person);

    let (reviewer, _) = api.agent_token_as("DEMO-1", AgentRole::Reviewer).await;
    let hook = permission(&api, &reviewer, "Bash: ls");
    let person = async {
        let q = wait_for(&api, 1).await;
        assert!(q[0]["detail"].is_null(), "{}", q[0]);
        answer(&api, json!({ "role": "reviewer", "answer": "allow" })).await;
    };
    tokio::join!(hook, person);
}

#[tokio::test]
async fn an_agent_that_waits_on_its_screen_is_listed_until_its_first_sign_of_life() {
    let (api, token) = setup().await;
    let (reviewer, grant) = api.agent_token_as("DEMO-1", AgentRole::Reviewer).await;
    // Spawned a minute ago; the profile gives it 20 seconds.
    let spawned = Timestamp(now().0 - 60_000);
    api.runners.tokens().spawned(grant, spawned, 20);
    let q = wait_for(&api, 1).await;
    assert_eq!(q[0]["text"], "The agent waits on its screen");
    assert_eq!(q[0]["role"], "reviewer");
    assert_eq!(
        q[0]["answers"],
        json!(["enter", "one", "two", "up", "down", "escape", "text"])
    );
    assert!(q[0]["lines"].is_null(), "no output yet: no lines");
    // A worker's sign of life does not end the reviewer's wait.
    api.call_with(
        &token,
        Method::POST,
        "/v1/tickets/DEMO-1/activity",
        Some(json!({ "signal": "hook:stop" })),
    )
    .await;
    assert_eq!(questions(&api).await.len(), 1);
    // The reviewer's first hook does.
    let (s, _) = api
        .call_with(
            &reviewer,
            Method::POST,
            "/v1/tickets/DEMO-1/activity",
            Some(json!({ "signal": "hook:pre_tool_use" })),
        )
        .await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    assert!(questions(&api).await.is_empty());
}

#[tokio::test]
async fn an_answer_needs_an_open_question_the_answer_fits() {
    let (api, token) = setup().await;
    let (s, v) = answer(&api, json!({ "answer": "enter" })).await;
    assert_eq!(s, StatusCode::CONFLICT, "nothing is asked: {v}");
    // An ask_human question takes a comment, not a key.
    api.call_with(
        &token,
        Method::POST,
        "/v1/tickets/DEMO-1/events",
        Some(json!({ "event": "ask_human", "text": "which db?" })),
    )
    .await;
    let q = wait_for(&api, 1).await;
    assert!(q[0]["answers"].is_null(), "a comment answers it");
    let (s, _) = answer(&api, json!({ "answer": "allow" })).await;
    assert_eq!(s, StatusCode::CONFLICT);
    assert_eq!(api.status("DEMO-1").await, "waiting");
}

#[tokio::test]
async fn a_key_for_a_dialog_without_a_hook_goes_to_the_terminal_or_is_refused() {
    let (api, token) = setup().await;
    api.call_with(
        &token,
        Method::POST,
        "/v1/tickets/DEMO-1/activity",
        Some(json!({ "signal": "hook:notification", "prompt": "Claude needs your permission" })),
    )
    .await;
    wait_for(&api, 1).await;
    // No process here: the keys have nowhere to go, and the question stays.
    let (s, v) = answer(&api, json!({ "answer": "allow" })).await;
    assert_eq!(s, StatusCode::CONFLICT, "{v}");
    assert_eq!(api.status("DEMO-1").await, "waiting");
    assert_eq!(questions(&api).await.len(), 1);
}

#[tokio::test]
async fn a_worker_that_handed_in_gets_a_denial_at_once_and_the_card_stays_in_review() {
    let (api, token) = setup().await;
    api.call_with(
        &token,
        Method::POST,
        "/v1/tickets/DEMO-1/events",
        Some(json!({ "event": "finished", "summary": toknila_core::handin::from_plain("done") })),
    )
    .await;
    assert_eq!(api.status("DEMO-1").await, "review");
    let (status, reply) = permission(&api, &token, "Bash: cargo xtask verify").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(reply["decision"], "deny");
    assert_eq!(
        reply["message"],
        toknila_core::question::HANDED_IN_DENIAL,
        "{reply}"
    );
    assert_eq!(api.status("DEMO-1").await, "review", "nobody was asked");
    assert!(questions(&api).await.is_empty());

    // A reviewer's dialog is not refused: it is the person's to decide.
    let (reviewer, _) = api.agent_token_as("DEMO-1", AgentRole::Reviewer).await;
    api.runners.set_permission_wait(Duration::from_millis(60));
    let (_, reply) = permission(&api, &reviewer, "Bash: ls").await;
    assert_eq!(reply, json!({ "decision": "ask" }));
}
