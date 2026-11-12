//! Agent tokens through the full router: what an agent may do with its run's token, what it may
//! not, and that the same events move a ticket the same way as a person's `tok sim`.

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};
use toknila_core::{AgentRole, RunId, TicketId};

use super::Api;
use crate::runner::Grant;
use crate::server::{AGENT_ROUTES, now};

impl Api {
    /// A token for the agent of `key`, as the runner issues it when a run starts.
    pub(super) async fn agent_token(&self, key: &str) -> (String, Grant) {
        self.agent_token_as(key, AgentRole::Worker).await
    }

    /// A token for the `role` run of `key`.
    pub(super) async fn agent_token_as(&self, key: &str, role: AgentRole) -> (String, Grant) {
        let id = self.get(&format!("/v1/tickets/{key}")).await["ticket"]["id"]
            .as_str()
            .and_then(|s| s.parse::<TicketId>().ok())
            .expect("ticket id");
        let grant = Grant {
            ticket: id,
            run: RunId::new(),
            role,
        };
        let token = self.runners.tokens().issue(grant, now()).expect("token");
        (token, grant)
    }

    pub(super) async fn demo_with(&self, tickets: &[&str]) {
        self.demo().await;
        for title in tickets {
            self.post(
                "/v1/chains/demo/tickets",
                json!({ "title": title, "subtasks": ["one"] }),
            )
            .await;
        }
    }
}

/// Every route of the API as `(METHOD, path with parameters filled in)`: the documented ones,
/// plus the three that are not in the OpenAPI document.
pub(super) fn every_route() -> Vec<(String, String)> {
    let mut routes: Vec<(String, String)> = Vec::new();
    let doc = serde_json::to_value(crate::http::v1().1).expect("openapi");
    for (path, item) in doc["paths"].as_object().expect("paths") {
        for method in item.as_object().expect("item").keys() {
            let filled = path
                .split('/')
                .map(|seg| if seg.starts_with('{') { "DEMO-1" } else { seg })
                .collect::<Vec<_>>()
                .join("/");
            routes.push((method.to_uppercase(), format!("/v1{filled}")));
        }
    }
    routes.push(("GET".into(), "/v1/stream".into()));
    routes.push(("GET".into(), "/v1/tickets/DEMO-1/runner/stream".into()));
    routes.push(("POST".into(), "/v1/shutdown".into()));
    routes
}

fn is_agent_route(method: &str, uri: &str) -> bool {
    method == "POST"
        && AGENT_ROUTES.iter().any(|template| {
            let filled = template.replace("{ticket}", "DEMO-1");
            filled == uri
        })
}

#[tokio::test]
async fn an_agent_token_is_refused_with_403_on_every_route_but_its_own_six() {
    let api = Api::new();
    api.demo_with(&["A"]).await;
    let (token, _) = api.agent_token("DEMO-1").await;

    let routes = every_route();
    let refused: Vec<_> = routes
        .iter()
        .filter(|(m, uri)| !is_agent_route(m, uri))
        .collect();
    assert!(
        refused.len() > 30,
        "the table is not vacuous: {}",
        refused.len()
    );
    for (method, uri) in &refused {
        let method = Method::from_bytes(method.as_bytes()).expect("method");
        let (status, body) = api
            .call_with(&token, method.clone(), uri, Some(json!({})))
            .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{method} {uri}: {body}");
        assert_eq!(body["code"], "forbidden", "{method} {uri}");
    }

    // The routes an agent has are open to it (the control: the probe can say yes).
    let (status, _) = api
        .call_with(
            &token,
            Method::POST,
            "/v1/tickets/DEMO-1/events",
            Some(json!({ "event": "started" })),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = api
        .call_with(
            &token,
            Method::POST,
            "/v1/tickets/DEMO-1/activity",
            Some(json!({ "signal": "hook:stop" })),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, body) = api
        .call_with(
            &token,
            Method::POST,
            "/v1/tickets/DEMO-1/instructions/claim",
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, json!({ "count": 0, "reason": "" }));
    // Nobody answers within the wait: the agent is told to show its own dialog.
    api.runners
        .set_permission_wait(std::time::Duration::from_millis(50));
    let (status, body) = api
        .call_with(
            &token,
            Method::POST,
            "/v1/usage/quota",
            Some(json!({ "windows": [] })),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    let (status, body) = api
        .call_with(
            &token,
            Method::POST,
            "/v1/tickets/DEMO-1/permission",
            Some(json!({ "tool": "Bash: ls" })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, json!({ "decision": "ask" }));
    let (status, body) = api
        .call_with(
            &token,
            Method::POST,
            "/v1/tickets/DEMO-1/pre-tool",
            Some(json!({ "tool": "Bash: ls" })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, json!({ "decision": "ask" }));
    assert_eq!(
        routes.iter().filter(|(m, u)| is_agent_route(m, u)).count(),
        6
    );
}

#[tokio::test]
async fn the_person_only_actions_by_name_refuse_an_agent_and_still_work_for_a_person() {
    let api = Api::new();
    api.demo_with(&["A"]).await;
    let (token, _) = api.agent_token("DEMO-1").await;
    api.post(
        "/v1/tickets/DEMO-1/events",
        json!({ "event": "open_thread", "kind": "design", "title": "screen" }),
    )
    .await;
    let person_only = [
        ("/v1/chains/demo/wildcards", json!({ "title": "w" })),
        ("/v1/tickets/DEMO-1/actions/approve", json!({})),
        ("/v1/tickets/DEMO-1/actions/reject", json!({ "text": "no" })),
        ("/v1/tickets/DEMO-1/actions/accept", json!({})),
        (
            "/v1/tickets/DEMO-1/actions/return",
            json!({ "text": "again" }),
        ),
        (
            "/v1/tickets/DEMO-1/actions/cancel",
            json!({ "text": "stop" }),
        ),
        ("/v1/tickets/DEMO-1/threads/1/approve", json!({})),
        (
            "/v1/tickets/DEMO-1/threads/1/changes",
            json!({ "text": "x" }),
        ),
        ("/v1/tickets/DEMO-1/comments", json!({ "text": "order" })),
    ];
    for (uri, body) in person_only {
        let (agent, _) = api
            .call_with(&token, Method::POST, uri, Some(body.clone()))
            .await;
        assert_eq!(agent, StatusCode::FORBIDDEN, "agent {uri}");
        let (person, v) = api.call(Method::POST, uri, Some(body)).await;
        assert_ne!(person, StatusCode::FORBIDDEN, "person {uri}: {v}");
        assert_ne!(person, StatusCode::UNAUTHORIZED, "person {uri}: {v}");
    }
    // Input into the terminal: the person's route, no runner here, so 409 for a person.
    let (agent, _) = api
        .call_with(
            &token,
            Method::POST,
            "/v1/tickets/DEMO-1/runner/input",
            None,
        )
        .await;
    assert_eq!(agent, StatusCode::FORBIDDEN);
}

/// The ticket's status, reason and checklist after each event: what `tok sim` would show.
async fn drive(api: &Api, token: &str, key: &str) -> Vec<Value> {
    let events = [
        json!({ "event": "started" }),
        json!({ "event": "ask_human", "text": "which framework?" }),
        json!({ "event": "subtask_done", "position": 1 }),
        json!({ "event": "note", "body": { "gist": "half way" } }),
    ];
    let mut seen = Vec::new();
    let uri = format!("/v1/tickets/{key}/events");
    let at = |v: &Value| {
        json!({
            "status": v["ticket"]["status"],
            "reason": v["ticket"]["status_reason"],
            "done_by": v["subtasks"][0]["done_by"],
            "comments": v["comments"].as_array().map(|c| c.iter()
                .map(|c| json!([c["author"], c["kind"], c["body"]["gist"]])).collect::<Vec<_>>()),
        })
    };
    for e in events {
        let (s, v) = api
            .call_with(token, Method::POST, &uri, Some(e.clone()))
            .await;
        assert_eq!(s, StatusCode::OK, "{e}: {v}");
        seen.push(at(&v));
    }
    api.post(
        &format!("/v1/tickets/{key}/comments"),
        json!({ "text": "Solid" }),
    )
    .await;
    for e in [json!({ "event": "finished", "summary": toknila_core::handin::from_plain("did X") })]
    {
        let (s, v) = api
            .call_with(token, Method::POST, &uri, Some(e.clone()))
            .await;
        assert_eq!(s, StatusCode::OK, "{e}: {v}");
        seen.push(at(&v));
    }
    seen
}

#[tokio::test]
async fn an_agent_token_moves_its_ticket_exactly_like_the_persons_sim_events() {
    let api = Api::new();
    api.demo_with(&["A", "B"]).await;
    let (token, _) = api.agent_token("DEMO-2").await;

    let by_person = drive(&api, super::TOKEN, "DEMO-1").await;
    let by_agent = drive(&api, &token, "DEMO-2").await;
    assert_eq!(by_agent, by_person);
    let statuses: Vec<_> = by_agent.iter().map(|s| s["status"].as_str()).collect();
    assert_eq!(
        statuses,
        [
            Some("running"),
            Some("waiting"),
            Some("waiting"),
            Some("waiting"),
            Some("review")
        ]
    );

    // And a crash, from a second run of the same agent.
    api.post(
        "/v1/tickets/DEMO-2/actions/return",
        json!({ "text": "again" }),
    )
    .await;
    let (s, v) = api
        .call_with(
            &token,
            Method::POST,
            "/v1/tickets/DEMO-2/events",
            Some(json!({ "event": "crashed", "detail": "exit 101" })),
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["ticket"]["status"], "blocked");
    assert_eq!(v["ticket"]["status_reason"]["reason"]["kind"], "crashed");
}

#[tokio::test]
async fn events_for_a_ticket_the_run_does_not_own_are_rejected_and_change_nothing() {
    let api = Api::new();
    api.demo_with(&["A", "B"]).await;
    let (token, _) = api.agent_token("DEMO-1").await;
    let events = [
        json!({ "event": "ask_human", "text": "hijack" }),
        json!({ "event": "finished", "summary": toknila_core::handin::from_plain("not mine") }),
        json!({ "event": "propose", "title": "sneaky" }),
        json!({ "event": "open_thread", "kind": "design", "title": "x" }),
    ];
    for e in events {
        let (s, v) = api
            .call_with(
                &token,
                Method::POST,
                "/v1/tickets/DEMO-2/events",
                Some(e.clone()),
            )
            .await;
        assert_eq!(s, StatusCode::FORBIDDEN, "{e}: {v}");
        assert_eq!(v["code"], "forbidden");
    }
    let (s, _) = api
        .call_with(
            &token,
            Method::POST,
            "/v1/tickets/DEMO-2/activity",
            Some(json!({ "signal": "hook:stop" })),
        )
        .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    assert_eq!(api.status("DEMO-2").await, "running");
    let other = api.get("/v1/tickets/DEMO-2").await;
    assert_eq!(other["comments"], json!([]));
    assert_eq!(
        api.get("/v1/tickets").await.as_array().map(Vec::len),
        Some(2)
    );
}

#[tokio::test]
async fn a_revoked_token_stops_working_and_a_made_up_one_never_did() {
    let api = Api::new();
    api.demo_with(&["A"]).await;
    let (token, grant) = api.agent_token("DEMO-1").await;
    let uri = "/v1/tickets/DEMO-1/events";
    let started = || Some(json!({ "event": "started" }));
    let (s, _) = api.call_with(&token, Method::POST, uri, started()).await;
    assert_eq!(s, StatusCode::OK);

    api.runners.tokens().revoke(grant.run);
    let (s, v) = api.call_with(&token, Method::POST, uri, started()).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED, "{v}");
    let (s, _) = api.call_with("made-up", Method::POST, uri, started()).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn hook_activity_is_recorded_per_ticket_and_changes_nothing_else() {
    let api = Api::new();
    api.demo_with(&["A"]).await;
    let (token, grant) = api.agent_token("DEMO-1").await;
    let before = api.get("/v1/tickets/DEMO-1").await;
    let (s, _) = api
        .call_with(
            &token,
            Method::POST,
            "/v1/tickets/DEMO-1/activity",
            Some(json!({ "signal": "hook:stop" })),
        )
        .await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    let last = api
        .runners
        .tokens()
        .last_activity(grant.ticket)
        .expect("activity");
    assert_eq!(last.signal, "hook:stop");
    assert_eq!(
        api.get("/v1/tickets/DEMO-1").await["ticket"],
        before["ticket"]
    );
}
