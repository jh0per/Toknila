//! The planner on the person's routes (TOKNILA-149): one route per action, and the caller the
//! token names decides through core's `access::may`. The planner plans its own project there,
//! is refused another project's things and everything its limits forbid with the words it always
//! had, and starts nothing; the person's use of the same routes does not change; a card's agent
//! reaches none of them.

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};

use super::Api;

fn error(v: &Value) -> &str {
    v["error"].as_str().unwrap_or_default()
}

#[tokio::test]
async fn the_planner_plans_its_own_project_on_the_persons_routes_and_no_other() {
    let api = Api::new();
    api.two_projects().await;
    let token = api.planner_token("DEMO").await;
    let call = |m: Method, uri: &'static str, body: Option<Value>| {
        let (api, token) = (&api, token.clone());
        async move { api.call_with(&token, m, uri, body).await }
    };

    let (s, epic) = call(
        Method::POST,
        "/v1/projects/DEMO/chains",
        Some(json!({ "name": "made", "goal": "by the planner" })),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED, "{epic}");
    assert_eq!(epic["chain"]["autostart"], false, "a planner's epic waits");
    let (s, one) = call(
        Method::POST,
        "/v1/chains/made/tickets",
        Some(json!({ "title": "One", "subtasks": ["s"], "at_end": true })),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED, "{one}");
    assert_eq!(one["ticket"]["key"], "DEMO-1");
    let (s, edited) = call(
        Method::PATCH,
        "/v1/tickets/DEMO-1",
        Some(json!({ "title": "One!", "add_subtasks": ["x"],
                     "edit_subtasks": [{ "position": 1, "text": "s!" }] })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{edited}");
    assert_eq!(edited["ticket"]["title"], "One!");
    let subtasks: Vec<&str> = edited["subtasks"]
        .as_array()
        .expect("subtasks")
        .iter()
        .filter_map(|t| t["text"].as_str())
        .collect();
    assert_eq!(subtasks, ["s!", "x"], "one command: renamed and appended");
    let (s, thread) = call(
        Method::POST,
        "/v1/tickets/DEMO-1/threads",
        Some(json!({ "kind": "design", "title": "Order" })),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED, "{thread}");
    assert_eq!(thread["thread"]["opened_by"], "agent");
    let (s, said) = call(
        Method::POST,
        "/v1/tickets/DEMO-1/threads/1/messages",
        Some(json!({ "text": "Which first?" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{said}");
    assert_eq!(said["messages"][0]["author"], "agent");
    let (s, shown) = call(Method::POST, "/v1/tickets/DEMO-1/show", None).await;
    assert_eq!(s, StatusCode::OK, "{shown}");
    let (s, kept) = call(
        Method::PUT,
        "/v1/projects/DEMO/suggestion",
        Some(json!({ "text": "Start One", "card": "DEMO-1", "action": "start" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{kept}");
    for read in [
        "/v1/projects/DEMO/plan",
        "/v1/projects/DEMO/plan/overview",
        "/v1/chains/made/plan",
        "/v1/tickets/DEMO-1/plan",
    ] {
        let (s, v) = call(Method::GET, read, None).await;
        assert_eq!(s, StatusCode::OK, "{read}: {v}");
    }
    // What it did is told in its conversation, as it was from its old routes.
    let told: Vec<String> = api.get("/v1/projects/DEMO/conversation").await["messages"]
        .as_array()
        .expect("messages")
        .iter()
        .filter_map(|m| m["text"].as_str().map(str::to_owned))
        .collect();
    assert!(
        told.iter().any(|t| t.starts_with("added epic ")),
        "{told:?}"
    );
    assert!(told.iter().any(|t| t.contains("DEMO-1")), "{told:?}");

    // The other project's things, by name and key: refused, and untouched.
    let foreign: [(Method, &'static str, Option<Value>); 13] = [
        (Method::GET, "/v1/projects/OTHER/plan", None),
        (Method::GET, "/v1/chains/theirs/plan", None),
        (Method::GET, "/v1/tickets/OTHER-1/plan", None),
        (
            Method::POST,
            "/v1/projects/OTHER/chains",
            Some(json!({ "name": "sneak" })),
        ),
        (
            Method::PATCH,
            "/v1/chains/theirs",
            Some(json!({ "goal": "mine" })),
        ),
        (
            Method::POST,
            "/v1/chains/theirs/tickets",
            Some(json!({ "title": "Sneak" })),
        ),
        (
            Method::PATCH,
            "/v1/tickets/OTHER-1",
            Some(json!({ "title": "Mine now" })),
        ),
        (
            Method::POST,
            "/v1/tickets/OTHER-1/threads",
            Some(json!({ "kind": "design", "title": "x" })),
        ),
        (
            Method::POST,
            "/v1/chains/theirs/edges",
            Some(json!({ "from": "OTHER-1", "to": "OTHER-1" })),
        ),
        (
            Method::POST,
            "/v1/chains/theirs/tickets/OTHER-1/move",
            Some(json!({ "anchor": "OTHER-1", "place": "before" })),
        ),
        (Method::POST, "/v1/tickets/OTHER-1/show", None),
        (
            Method::PUT,
            "/v1/projects/OTHER/suggestion",
            Some(json!({ "text": "x" })),
        ),
        (
            Method::POST,
            "/v1/projects/OTHER/design-pins",
            Some(
                json!({ "design": "x", "region": { "x": 1, "y": 1, "w": 1, "h": 1 },
                         "text": "?" }),
            ),
        ),
    ];
    for (m, uri, body) in foreign {
        let (s, e) = call(m.clone(), uri, body).await;
        assert_eq!(s, StatusCode::FORBIDDEN, "{m} {uri}: {e}");
        assert!(error(&e).contains("another project"), "{m} {uri}: {e}");
    }
    let (_, theirs) = api.call(Method::GET, "/v1/tickets/OTHER-1", None).await;
    assert_eq!(theirs["ticket"]["title"], "Not yours", "untouched");
}

#[tokio::test]
async fn the_planners_limits_hold_on_the_persons_routes_with_the_words_it_always_had() {
    let api = Api::new();
    api.two_projects().await;
    for title in ["A", "B"] {
        api.post("/v1/chains/plan/tickets", json!({ "title": title }))
            .await;
    }
    api.post("/v1/projects/DEMO/chains", json!({ "name": "auto" }))
        .await;
    let token = api.planner_token("DEMO").await;
    let call = |m: Method, uri: &'static str, body: Option<Value>| {
        let (api, token) = (&api, token.clone());
        async move { api.call_with(&token, m, uri, body).await }
    };
    let refused = |s: StatusCode, e: &Value, words: &str, what: &str| {
        assert_eq!(s, StatusCode::FORBIDDEN, "{what}: {e}");
        assert!(error(e).contains(words), "{what}: {e}");
    };

    // An epic that starts its stories by itself: no story, no dependency, no move.
    let (s, e) = call(
        Method::POST,
        "/v1/chains/auto/tickets",
        Some(json!({ "title": "C" })),
    )
    .await;
    refused(s, &e, "only the owner starts stories", "a story in auto");
    let (s, e) = call(
        Method::POST,
        "/v1/chains/auto/edges",
        Some(json!({ "from": "DEMO-1", "to": "DEMO-2" })),
    )
    .await;
    refused(s, &e, "only the owner starts stories", "an edge in auto");
    let (s, e) = call(
        Method::POST,
        "/v1/tickets/DEMO-1/move-to-chain?from=plan",
        Some(json!({ "chain": "auto" })),
    )
    .await;
    refused(s, &e, "only the owner starts stories", "a move into auto");

    // The owner's settings: autostart, review, the story's agent, a release-wait dependency.
    let owners: [(Method, &'static str, Value); 5] = [
        (
            Method::POST,
            "/v1/projects/DEMO/chains",
            json!({ "name": "fast", "autostart": true }),
        ),
        (
            Method::PATCH,
            "/v1/chains/plan",
            json!({ "autostart": true }),
        ),
        (
            Method::POST,
            "/v1/chains/plan/tickets",
            json!({ "title": "C", "profile": "sim" }),
        ),
        (
            Method::PATCH,
            "/v1/tickets/DEMO-1",
            json!({ "review_mode": "human" }),
        ),
        (
            Method::POST,
            "/v1/chains/plan/edges",
            json!({ "from": "DEMO-1", "to": "DEMO-2", "kind": "after_release" }),
        ),
    ];
    for (m, uri, body) in owners {
        let (s, e) = call(m.clone(), uri, Some(body)).await;
        refused(s, &e, "only the owner sets", &format!("{m} {uri}"));
    }
    assert_eq!(
        api.get("/v1/chains/plan").await["chain"]["autostart"],
        false
    );

    // A story that started is not the planner's to rewrite; it starts nothing itself.
    let (s, e) = call(
        Method::POST,
        "/v1/tickets/DEMO-1/actions/start",
        Some(json!({})),
    )
    .await;
    refused(s, &e, "may only plan", "start");
    assert_eq!(
        api.status("DEMO-1").await,
        "ready",
        "the refused start did nothing"
    );
    api.post("/v1/tickets/DEMO-1/actions/start", json!({}))
        .await;
    let (s, e) = call(
        Method::PATCH,
        "/v1/tickets/DEMO-1",
        Some(json!({ "title": "Late" })),
    )
    .await;
    refused(
        s,
        &e,
        "edits only stories that have not started",
        "a started story",
    );

    // The epic the planner names as the story's own must be it.
    api.post(
        "/v1/projects/DEMO/chains",
        json!({ "name": "side", "autostart": false }),
    )
    .await;
    let (s, e) = call(
        Method::POST,
        "/v1/tickets/DEMO-2/move-to-chain?from=side",
        Some(json!({ "chain": "plan" })),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{e}");
    assert_eq!(error(&e), "DEMO-2 is not in epic side");
}

#[tokio::test]
async fn on_the_roadmap_the_persons_story_route_keeps_the_planner_to_the_epic_it_made() {
    let api = Api::new();
    api.two_projects().await;
    let token = api.planner_token("DEMO").await;
    api.post(
        "/v1/projects/DEMO/conversation",
        json!({ "text": "add hiding a project", "about": { "kind": "roadmap" } }),
    )
    .await;
    let (s, e) = api
        .call_with(
            &token,
            Method::POST,
            "/v1/chains/plan/tickets",
            Some(json!({ "title": "Hide" })),
        )
        .await;
    assert_eq!(s, StatusCode::FORBIDDEN, "{e}");
    assert!(error(&e).contains("the person is on the roadmap"), "{e}");
    let (s, epic) = api
        .call_with(
            &token,
            Method::POST,
            "/v1/projects/DEMO/chains",
            Some(json!({ "title": "Hiding" })),
        )
        .await;
    assert_eq!(s, StatusCode::CREATED, "{epic}");
    let name = epic["chain"]["name"].as_str().expect("name").to_owned();
    let (s, v) = api
        .call_with(
            &token,
            Method::POST,
            &format!("/v1/chains/{name}/tickets"),
            Some(json!({ "title": "Hide" })),
        )
        .await;
    assert_eq!(s, StatusCode::CREATED, "the epic made in the turn: {v}");
}

#[tokio::test]
async fn the_person_reads_the_plan_but_never_speaks_for_the_assistant_and_a_cards_agent_reaches_none()
 {
    let api = Api::new();
    api.two_projects().await;
    api.post("/v1/chains/plan/tickets", json!({ "title": "A" }))
        .await;
    let (s, plan) = api.call(Method::GET, "/v1/projects/DEMO/plan", None).await;
    assert_eq!(s, StatusCode::OK, "{plan}");
    assert_eq!(plan["project"]["key"], "DEMO");
    let (s, card) = api.call(Method::GET, "/v1/tickets/DEMO-1/plan", None).await;
    assert_eq!(
        (s, &card["ticket"]["key"]),
        (StatusCode::OK, &json!("DEMO-1"))
    );
    for (m, uri, body) in [
        (
            Method::PUT,
            "/v1/projects/DEMO/suggestion",
            json!({ "text": "x" }),
        ),
        (
            Method::POST,
            "/v1/projects/DEMO/design-pins",
            json!({ "design": "x", "region": { "x": 1, "y": 1, "w": 1, "h": 1 }, "text": "?" }),
        ),
    ] {
        let (s, e) = api.call(m.clone(), uri, Some(body)).await;
        assert_eq!(s, StatusCode::FORBIDDEN, "{m} {uri}: {e}");
        assert_eq!(error(&e), "these routes are for a project's planner");
    }
    // The person's edit still lands as theirs, with the owner's fields.
    let (s, edited) = api
        .call(
            Method::PATCH,
            "/v1/tickets/DEMO-1",
            Some(json!({ "review_mode": "human", "title": "A!" })),
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{edited}");
    assert_eq!(edited["ticket"]["review_mode"], "human");

    let (worker, _) = api.agent_token("DEMO-1").await;
    for (m, uri) in [
        (Method::GET, "/v1/projects/DEMO/plan"),
        (Method::GET, "/v1/tickets/DEMO-1/plan"),
        (Method::PATCH, "/v1/tickets/DEMO-1"),
        (Method::POST, "/v1/tickets/DEMO-1/threads"),
        (Method::POST, "/v1/chains/plan/tickets"),
    ] {
        let (s, e) = api
            .call_with(&worker, m.clone(), uri, Some(json!({})))
            .await;
        assert_eq!(s, StatusCode::FORBIDDEN, "worker {m} {uri}: {e}");
    }
}
