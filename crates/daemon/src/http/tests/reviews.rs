//! Review through the full router with the simulated reviewer: a person's accept and return, the
//! reviewer agent's verdict through its own token, and what each role's token may send.

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};
use toknila_core::AgentRole;

use super::Api;

async fn finish(api: &Api, key: &str) {
    api.post(
        &format!("/v1/tickets/{key}/events"),
        json!({ "event": "finished", "summary": "done" }),
    )
    .await;
}

fn rounds(detail: &Value) -> Vec<(u64, String, String, String)> {
    detail["review"]["rounds"]
        .as_array()
        .expect("rounds")
        .iter()
        .map(|r| {
            let s = |k: &str| r[k].as_str().unwrap_or_default().to_owned();
            (
                r["number"].as_u64().expect("number"),
                s("reviewer"),
                s("verdict"),
                s("comment"),
            )
        })
        .collect()
}

fn actions(detail: &Value) -> Vec<String> {
    detail["allowed_actions"]
        .as_array()
        .expect("allowed_actions")
        .iter()
        .map(|a| a.as_str().expect("action").to_owned())
        .collect()
}

#[tokio::test]
async fn human_mode_the_card_waits_for_a_person_who_accepts_with_remarks() {
    let api = Api::new();
    api.demo_with(&["A"]).await;
    finish(&api, "DEMO-1").await;

    let t = api.get("/v1/tickets/DEMO-1").await;
    assert_eq!(t["review"]["mode"], "human");
    assert_eq!(t["review"]["reviewer"], "sim-reviewer");
    assert_eq!(t["review"]["awaiting"], json!({ "by": "person" }));
    assert_eq!(actions(&t), ["accept", "return", "cancel"]);

    let done = api
        .post(
            "/v1/tickets/DEMO-1/actions/accept",
            json!({ "text": "good" }),
        )
        .await;
    assert_eq!(done["ticket"]["status"], "merged");
    assert_eq!(
        rounds(&done),
        [(1, "human".into(), "approve".into(), "good".into())]
    );
    assert_eq!(actions(&done), ["reopen"]);
}

#[tokio::test]
async fn agent_mode_only_the_reviewers_token_may_give_the_verdict() {
    let api = Api::new();
    api.demo_with(&["A"]).await;
    api.call(
        Method::PATCH,
        "/v1/projects/DEMO",
        Some(json!({ "review_mode": "agent" })),
    )
    .await;
    finish(&api, "DEMO-1").await;
    let t = api.get("/v1/tickets/DEMO-1").await;
    assert_eq!(t["review"]["mode"], "agent");
    assert_eq!(
        t["review"]["awaiting"],
        json!({ "by": "agent", "decides": true })
    );

    let verdict = json!({ "event": "review_verdict", "verdict": "request_changes",
        "comment": "handle the empty list" });
    let (worker, _) = api.agent_token("DEMO-1").await;
    let (reviewer, _) = api.agent_token_as("DEMO-1", AgentRole::Reviewer).await;
    let events = "/v1/tickets/DEMO-1/events";

    // The worker cannot judge its own work; the reviewer cannot report work.
    let (status, body) = api
        .call_with(&worker, Method::POST, events, Some(verdict.clone()))
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    let (status, body) = api
        .call_with(
            &reviewer,
            Method::POST,
            events,
            Some(json!({ "event": "finished", "summary": "x" })),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(api.status("DEMO-1").await, "review", "nothing moved");

    // Requested changes without remarks are refused, with them the card goes back.
    let (status, _) = api
        .call_with(
            &reviewer,
            Method::POST,
            events,
            Some(json!({ "event": "review_verdict", "verdict": "request_changes" })),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, body) = api
        .call_with(&reviewer, Method::POST, events, Some(verdict))
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["ticket"]["status"], "running");
    assert_eq!(
        rounds(&body),
        [(
            1,
            "agent".into(),
            "request_changes".into(),
            "handle the empty list".into()
        )]
    );

    // And the verdict is only heard while the card waits for it.
    let (status, _) = api
        .call_with(
            &reviewer,
            Method::POST,
            events,
            Some(json!({ "event": "review_verdict", "verdict": "approve" })),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);

    // The reviewer's token has no other routes than a worker's two.
    let (status, _) = api
        .call_with(
            &reviewer,
            Method::POST,
            "/v1/tickets/DEMO-1/actions/accept",
            Some(json!({})),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn agent_mode_an_approval_merges_the_card() {
    let api = Api::new();
    api.demo_with(&["A"]).await;
    api.call(
        Method::PATCH,
        "/v1/tickets/DEMO-1",
        Some(json!({ "review_mode": "agent" })),
    )
    .await;
    finish(&api, "DEMO-1").await;
    let (reviewer, _) = api.agent_token_as("DEMO-1", AgentRole::Reviewer).await;
    let (status, body) = api
        .call_with(
            &reviewer,
            Method::POST,
            "/v1/tickets/DEMO-1/events",
            Some(json!({ "event": "review_verdict", "verdict": "approve", "comment": "fine" })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["ticket"]["status"], "merged");
    assert_eq!(
        rounds(&body),
        [(1, "agent".into(), "approve".into(), "fine".into())]
    );
}

#[tokio::test]
async fn both_mode_the_agent_speaks_and_the_person_decides_with_its_remarks_in_view() {
    let api = Api::new();
    api.demo_with(&["A"]).await;
    let (status, chain) = api
        .call(
            Method::PATCH,
            "/v1/chains/demo",
            Some(json!({ "review_mode": "both", "reviewer": "strict" })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{chain}");
    assert_eq!(chain["chain"]["review_mode"], "both");
    assert_eq!(chain["chain"]["reviewer"], "strict");
    finish(&api, "DEMO-1").await;
    let t = api.get("/v1/tickets/DEMO-1").await;
    assert_eq!(t["review"]["reviewer"], "strict", "the chain's setting");

    // A reviewer profile that is no simulation would start a process; this one is not
    // known, so nothing here runs it and the verdict comes from the token below.
    let (reviewer, _) = api.agent_token_as("DEMO-1", AgentRole::Reviewer).await;
    let (_, body) = api
        .call_with(
            &reviewer,
            Method::POST,
            "/v1/tickets/DEMO-1/events",
            Some(json!({ "event": "review_verdict", "verdict": "approve", "comment": "ok" })),
        )
        .await;
    assert_eq!(body["ticket"]["status"], "review", "a person still decides");
    assert_eq!(body["review"]["awaiting"], json!({ "by": "person" }));

    let back = api
        .post(
            "/v1/tickets/DEMO-1/actions/return",
            json!({ "text": "still fix the look" }),
        )
        .await;
    assert_eq!(back["ticket"]["status"], "running");
    let kinds: Vec<_> = rounds(&back).into_iter().map(|r| (r.1, r.2)).collect();
    assert_eq!(
        kinds,
        [
            ("agent".to_owned(), "approve".to_owned()),
            ("human".to_owned(), "request_changes".to_owned())
        ]
    );
}

#[tokio::test]
async fn the_settings_resolve_ticket_then_chain_then_project_and_a_blank_reviewer_clears_it() {
    let api = Api::new();
    api.demo_with(&["A"]).await;
    let mode = |t: Value| (t["review"]["mode"].clone(), t["review"]["reviewer"].clone());
    let patch = |uri: &'static str, body: Value| {
        let api = &api;
        async move {
            let (s, v) = api.call(Method::PATCH, uri, Some(body)).await;
            assert_eq!(s, StatusCode::OK, "{uri}: {v}");
        }
    };

    patch(
        "/v1/projects/DEMO",
        json!({ "review_mode": "both", "reviewer": "p" }),
    )
    .await;
    assert_eq!(
        mode(api.get("/v1/tickets/DEMO-1").await),
        (json!("both"), json!("p"))
    );
    patch(
        "/v1/chains/demo",
        json!({ "review_mode": "agent", "reviewer": "c" }),
    )
    .await;
    assert_eq!(
        mode(api.get("/v1/tickets/DEMO-1").await),
        (json!("agent"), json!("c"))
    );
    patch(
        "/v1/tickets/DEMO-1",
        json!({ "review_mode": "human", "reviewer": "t" }),
    )
    .await;
    assert_eq!(
        mode(api.get("/v1/tickets/DEMO-1").await),
        (json!("human"), json!("t"))
    );
    patch("/v1/tickets/DEMO-1", json!({ "reviewer": "" })).await;
    assert_eq!(
        mode(api.get("/v1/tickets/DEMO-1").await),
        (json!("human"), json!("c")),
        "blank clears the ticket's reviewer"
    );
    let project = api.get("/v1/projects/DEMO").await;
    assert_eq!(project["default_review_mode"], "both");
    assert_eq!(project["default_reviewer"], "p");

    // An unknown ticket and an unknown mode are told apart from success.
    let (s, _) = api
        .call(
            Method::PATCH,
            "/v1/chains/nope",
            Some(json!({ "review_mode": "human" })),
        )
        .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (s, _) = api
        .call(
            Method::PATCH,
            "/v1/chains/demo",
            Some(json!({ "review_mode": "robot" })),
        )
        .await;
    assert!(s.is_client_error(), "{s}");
}

#[tokio::test]
async fn a_reviewers_output_is_read_through_the_role_selector_and_is_empty_without_one() {
    let api = Api::new();
    api.demo_with(&["A"]).await;
    for role in ["", "?role=reviewer"] {
        let (status, _) = api
            .call(
                Method::GET,
                &format!("/v1/tickets/DEMO-1/runner/scrollback{role}"),
                None,
            )
            .await;
        assert_eq!(
            status,
            StatusCode::NOT_FOUND,
            "no runner ran, role {role:?}"
        );
    }
}
