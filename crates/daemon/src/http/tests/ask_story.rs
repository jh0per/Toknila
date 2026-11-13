//! A person asks the assistant about a story through the full router. The planner is a fake: the
//! test calls the planner's own `say` with its token, as its MCP tool does.

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};

use super::Api;

async fn setup(api: &Api) {
    api.two_projects().await;
    api.post(
        "/v1/chains/plan/tickets",
        json!({ "title": "Login screen", "prompt": "old prompt", "acceptance": ["Errors are shown"] }),
    )
    .await;
}

async fn comments(api: &Api) -> Vec<Value> {
    api.get("/v1/tickets/DEMO-1").await["comments"]
        .as_array()
        .expect("comments")
        .clone()
}

#[tokio::test]
async fn a_question_carries_the_record_to_the_assistant_and_its_answer_is_a_comment_that_changes_nothing()
 {
    let api = Api::new();
    setup(&api).await;
    let planner = api.planner_token("DEMO").await;
    let before = api.get("/v1/tickets/DEMO-1").await["ticket"].clone();

    let reply = api
        .post(
            "/v1/tickets/DEMO-1/ask",
            json!({ "text": "  шо треба зробити?  " }),
        )
        .await;
    let line = reply["line"].as_str().expect("line");
    for want in [
        "Question on story DEMO-1: шо треба зробити?",
        "Title: Login screen",
        "Prompt: old prompt",
        "Acceptance: 1. Errors are shown",
        "Status: ",
        "Hand-in: none",
    ] {
        assert!(line.contains(want), "missing {want:?} in {line}");
    }
    assert!(!line.contains('\n'), "one line for a terminal: {line}");
    assert_eq!(reply["comment"]["kind"], "question");
    assert_eq!(reply["comment"]["author"], "human");
    assert_eq!(reply["comment"]["body"]["gist"], "шо треба зробити?");
    assert!(reply["comment"]["delivered_at"].is_null());

    // Asking queues nothing for the agent: the question is no instruction.
    assert!(
        comments(&api)
            .await
            .iter()
            .all(|c| c["kind"] != "instruction"),
        "no instruction was queued"
    );
    assert!(api.get("/v1/tickets/DEMO-1").await["pending"].is_null());

    // The assistant answers with `say` about the story: its words land in the story's chat.
    let (status, said) = api
        .call_with(
            &planner,
            Method::POST,
            "/v1/planner/say",
            Some(json!({ "text": "Open it on a phone, then press Accept.", "about": "DEMO-1" })),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{said}");
    let all = comments(&api).await;
    let last = all.last().expect("a comment");
    assert_eq!(last["author"], "assistant");
    assert_eq!(last["kind"], "report");
    assert_eq!(
        last["body"]["gist"],
        "Open it on a phone, then press Accept."
    );
    assert_eq!(all.len(), 2, "the question and the answer, nothing else");

    // The story is as it was.
    let after = api.get("/v1/tickets/DEMO-1").await["ticket"].clone();
    for field in ["status", "prompt", "acceptance", "title"] {
        assert_eq!(before[field], after[field], "{field} moved");
    }
}

#[tokio::test]
async fn an_empty_question_is_refused_and_an_unknown_story_is_not_found() {
    let api = Api::new();
    setup(&api).await;
    let (status, _) = api
        .call(
            Method::POST,
            "/v1/tickets/DEMO-1/ask",
            Some(json!({ "text": "   " })),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = api
        .call(
            Method::POST,
            "/v1/tickets/DEMO-99/ask",
            Some(json!({ "text": "hello" })),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(comments(&api).await.is_empty());
}
