//! A planner's token through the full router: it reaches the planner routes and no other, it is
//! kept to its own project, and nothing it does starts a card. The owner's routes for the planner
//! (start, stop, the terminal) are a person's and refuse every agent token.

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};
use toknila_core::{AgentRole, ProjectId, RunId, planner};

use super::Api;
use super::agent_tokens::every_route;
use crate::runner::Grant;
use crate::server::{PLANNER_ROUTES, now};

impl Api {
    /// A token for the planner of `project`, as the runner issues it when a planner starts.
    pub(super) async fn planner_token(&self, project: &str) -> String {
        let id = self.get(&format!("/v1/projects/{project}")).await["id"]
            .as_str()
            .and_then(|s| s.parse::<ProjectId>().ok())
            .expect("project id");
        let grant = Grant {
            ticket: planner::run_key(id),
            run: RunId::new(),
            role: AgentRole::Planner,
        };
        self.runners.tokens().issue(grant, now()).expect("token")
    }

    /// DEMO with a chain that does not autostart, and another project with a chain of its own.
    pub(super) async fn two_projects(&self) {
        self.post("/v1/projects", json!({ "key": "DEMO", "name": "Demo" }))
            .await;
        self.post(
            "/v1/projects/DEMO/chains",
            json!({ "name": "plan", "autostart": false }),
        )
        .await;
        self.post("/v1/projects", json!({ "key": "OTHER", "name": "Other" }))
            .await;
        self.post(
            "/v1/projects/OTHER/chains",
            json!({ "name": "theirs", "autostart": false }),
        )
        .await;
        self.post("/v1/chains/theirs/tickets", json!({ "title": "Not yours" }))
            .await;
    }
}

fn is_planner_route(method: &str, uri: &str) -> bool {
    PLANNER_ROUTES.iter().any(|(m, template)| {
        let filled: String = template
            .split('/')
            .map(|seg| if seg.starts_with('{') { "DEMO-1" } else { seg })
            .collect::<Vec<_>>()
            .join("/");
        *m == method && filled == uri
    })
}

#[tokio::test]
async fn a_planner_token_is_refused_with_403_on_every_route_but_its_own_twelve() {
    let api = Api::new();
    api.two_projects().await;
    let token = api.planner_token("DEMO").await;

    let routes = every_route();
    let refused: Vec<_> = routes
        .iter()
        .filter(|(m, uri)| !is_planner_route(m, uri))
        .collect();
    assert!(refused.len() > 40, "not vacuous: {}", refused.len());
    for (method, uri) in &refused {
        let m = Method::from_bytes(method.as_bytes()).expect("method");
        let (status, body) = api.call_with(&token, m.clone(), uri, Some(json!({}))).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{m} {uri}: {body}");
        assert_eq!(body["code"], "forbidden", "{m} {uri}");
    }
    // The owner's calls by name, to be sure the names are among them.
    for (m, uri) in [
        ("POST", "/v1/tickets/DEMO-1/actions/start"),
        ("POST", "/v1/tickets/DEMO-1/actions/pause"),
        ("POST", "/v1/tickets/DEMO-1/actions/resume"),
        ("POST", "/v1/tickets/DEMO-1/actions/cancel"),
        ("POST", "/v1/tickets/DEMO-1/actions/accept"),
        ("POST", "/v1/tickets/DEMO-1/actions/return"),
        ("POST", "/v1/tickets/DEMO-1/actions/reopen"),
        ("POST", "/v1/tickets/DEMO-1/actions/approve"),
        ("POST", "/v1/tickets/DEMO-1/events"),
        ("POST", "/v1/tickets/DEMO-1/threads/1/approve"),
        ("POST", "/v1/projects/DEMO/planner/start"),
        ("POST", "/v1/projects/DEMO/planner/stop"),
        ("POST", "/v1/projects/DEMO/planner/input"),
        ("PATCH", "/v1/projects/DEMO"),
    ] {
        let method = Method::from_bytes(m.as_bytes()).expect("method");
        let (status, _) = api.call_with(&token, method, uri, Some(json!({}))).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{m} {uri}");
    }
    // The control: its own routes are open to it.
    let (status, plan) = api
        .call_with(&token, Method::GET, "/v1/planner/plan", None)
        .await;
    assert_eq!(status, StatusCode::OK, "{plan}");
    assert_eq!(
        routes
            .iter()
            .filter(|(m, u)| is_planner_route(m, u))
            .count(),
        PLANNER_ROUTES.len(),
        "every planner route is in the OpenAPI document"
    );
}

#[tokio::test]
async fn the_planner_routes_refuse_a_person_and_a_cards_agent() {
    let api = Api::new();
    api.two_projects().await;
    api.post("/v1/chains/plan/tickets", json!({ "title": "A" }))
        .await;
    let (worker, _) = api.agent_token("DEMO-1").await;
    for (who, token) in [("worker", worker.as_str()), ("person", super::TOKEN)] {
        let (status, body) = api
            .call_with(token, Method::GET, "/v1/planner/plan", None)
            .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{who}: {body}");
    }
}

#[tokio::test]
async fn a_planner_plans_its_own_project_and_cannot_touch_another() {
    let api = Api::new();
    api.two_projects().await;
    let token = api.planner_token("DEMO").await;
    let call = |m: Method, uri: &'static str, body: Option<Value>| {
        let (api, token) = (&api, token.clone());
        async move { api.call_with(&token, m, uri, body).await }
    };

    let (s, chain) = call(
        Method::POST,
        "/v1/planner/chains",
        Some(json!({ "name": "made", "goal": "by the planner" })),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED, "{chain}");
    assert_eq!(chain["chain"]["autostart"], false);
    let (s, card) = call(
        Method::POST,
        "/v1/planner/chains/made/cards",
        Some(json!({ "title": "One", "prompt": "p", "acceptance": ["a"], "subtasks": ["s"] })),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED, "{card}");
    assert_eq!(card["ticket"]["key"], "DEMO-1");
    let (s, two) = call(
        Method::POST,
        "/v1/planner/chains/made/cards",
        Some(json!({ "title": "Two", "after": ["DEMO-1"] })),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED, "{two}");
    let (s, edited) = call(
        Method::PATCH,
        "/v1/planner/cards/DEMO-2",
        Some(json!({ "title": "Two!", "add_subtasks": ["x"] })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{edited}");
    assert_eq!(edited["ticket"]["title"], "Two!");
    let (s, thread) = call(
        Method::POST,
        "/v1/planner/cards/DEMO-1/threads",
        Some(json!({ "kind": "design", "title": "Order" })),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED, "{thread}");
    assert_eq!(thread["thread"]["opened_by"], "agent");
    let (s, said) = call(
        Method::POST,
        "/v1/planner/cards/DEMO-1/threads/1/messages",
        Some(json!({ "text": "Which first?" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{said}");
    assert_eq!(said["messages"][0]["author"], "agent");

    // The plan shows this project only.
    let (_, plan) = call(Method::GET, "/v1/planner/plan", None).await;
    assert_eq!(plan["project"]["key"], "DEMO");
    let names: Vec<&str> = plan["chains"]
        .as_array()
        .expect("chains")
        .iter()
        .filter_map(|c| c["chain"]["name"].as_str())
        .collect();
    assert_eq!(names, ["plan", "made"]);

    // The other project's chain and card are off limits, by name, key and id; the same calls
    // succeed on its own (the control).
    let foreign: Vec<(Method, &str, Option<Value>)> = vec![
        (
            Method::POST,
            "/v1/planner/chains/theirs/cards",
            Some(json!({ "title": "Sneak" })),
        ),
        (
            Method::PATCH,
            "/v1/planner/cards/OTHER-1",
            Some(json!({ "title": "Mine now" })),
        ),
        (
            Method::POST,
            "/v1/planner/cards/OTHER-1/threads",
            Some(json!({ "kind": "design", "title": "x" })),
        ),
        (
            Method::POST,
            "/v1/planner/chains/theirs/edges",
            Some(json!({ "from": "OTHER-1", "to": "OTHER-1" })),
        ),
        (
            Method::POST,
            "/v1/planner/chains/theirs/cards/OTHER-1/move",
            Some(json!({ "anchor": "OTHER-1", "place": "before" })),
        ),
    ];
    for (m, uri, body) in foreign {
        let (s, e) = call(m.clone(), uri, body).await;
        assert_eq!(s, StatusCode::FORBIDDEN, "{m} {uri}: {e}");
        assert!(
            e["error"]
                .as_str()
                .unwrap_or_default()
                .contains("another project"),
            "{e}"
        );
    }
    let (_, theirs) = api.call(Method::GET, "/v1/tickets/OTHER-1", None).await;
    assert_eq!(theirs["ticket"]["title"], "Not yours", "untouched");
    let (s, _) = call(
        Method::PATCH,
        "/v1/planner/cards/DEMO-1",
        Some(json!({ "title": "One!" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "the control: its own card");
}

#[tokio::test]
async fn show_card_journals_one_window_event_for_a_card_of_its_own_project_only() {
    let api = Api::new();
    api.two_projects().await;
    api.post("/v1/chains/plan/tickets", json!({ "title": "A" }))
        .await;
    let token = api.planner_token("DEMO").await;
    let window_events = |events: Value| -> Vec<Value> {
        events
            .as_array()
            .expect("events")
            .iter()
            .filter(|e| e["entity"] == "window")
            .cloned()
            .collect()
    };
    let project_id = api.get("/v1/projects/DEMO").await["id"].clone();

    // Another project's card: refused, and nothing is journaled.
    let (s, e) = api
        .call_with(&token, Method::POST, "/v1/planner/cards/OTHER-1/show", None)
        .await;
    assert_eq!(s, StatusCode::FORBIDDEN, "{e}");
    assert!(window_events(api.get("/v1/events?since=0").await).is_empty());
    let (s, e) = api
        .call_with(&token, Method::POST, "/v1/planner/cards/DEMO-9/show", None)
        .await;
    assert_eq!(s, StatusCode::NOT_FOUND, "{e}");

    // Its own card: answered with the card, and one event in the stream.
    let (s, card) = api
        .call_with(&token, Method::POST, "/v1/planner/cards/DEMO-1/show", None)
        .await;
    assert_eq!(s, StatusCode::OK, "{card}");
    assert_eq!(card["ticket"]["key"], "DEMO-1");
    let events = window_events(api.get("/v1/events?since=0").await);
    assert_eq!(events.len(), 1, "{events:?}");
    let e = &events[0];
    assert_eq!(
        (&e["kind"], &e["actor"]),
        (&json!("show_card"), &json!("agent"))
    );
    assert_eq!(e["entity_id"], project_id);
    assert_eq!(
        e["payload"],
        json!({ "project": project_id, "project_key": "DEMO", "chain": "plan", "ticket": "DEMO-1" })
    );
}

#[tokio::test]
async fn nothing_a_planner_does_starts_a_card_even_in_a_project_with_agents() {
    let api = Api::new();
    api.two_projects().await;
    let token = api.planner_token("DEMO").await;
    for title in ["A", "B"] {
        let (s, v) = api
            .call_with(
                &token,
                Method::POST,
                "/v1/planner/chains/plan/cards",
                Some(json!({ "title": title })),
            )
            .await;
        assert_eq!(s, StatusCode::CREATED, "{v}");
    }
    assert_eq!(api.status("DEMO-1").await, "ready");
    assert_eq!(
        api.status("DEMO-2").await,
        "backlog",
        "the second card waits for the first, which nobody started"
    );
    let (s, e) = api
        .call_with(
            &token,
            Method::POST,
            "/v1/tickets/DEMO-1/actions/start",
            Some(json!({})),
        )
        .await;
    assert_eq!(
        (s, e["code"].as_str()),
        (StatusCode::FORBIDDEN, Some("forbidden"))
    );
    assert_eq!(
        api.status("DEMO-1").await,
        "ready",
        "the refused start did nothing"
    );

    // A chain the owner made to start its cards by itself refuses the planner's graph changes.
    api.post("/v1/projects/DEMO/chains", json!({ "name": "auto" }))
        .await;
    let (s, e) = api
        .call_with(
            &token,
            Method::POST,
            "/v1/planner/chains/auto/cards",
            Some(json!({ "title": "C" })),
        )
        .await;
    assert_eq!(s, StatusCode::FORBIDDEN, "{e}");
    assert!(
        e["error"]
            .as_str()
            .unwrap_or_default()
            .contains("only the owner starts cards")
    );
    assert_eq!(
        api.get("/v1/tickets").await.as_array().map(Vec::len),
        Some(3),
        "no card was added to it (two of ours and the other project's)"
    );
}

#[tokio::test]
async fn show_plan_lists_the_designs_with_paths_and_show_design_answers_one() {
    use crate::designs::Designs;
    use crate::designs::tests::PNG;
    use toknila_core::design::{self, DesignKind};

    let api = Api::new();
    api.two_projects().await;
    let project = |key: &str| {
        let key = key.to_owned();
        let api = &api;
        async move {
            api.get(&format!("/v1/projects/{key}")).await["id"]
                .as_str()
                .and_then(|s| s.parse::<ProjectId>().ok())
                .expect("project id")
        }
    };
    let (demo, other) = (project("DEMO").await, project("OTHER").await);
    let home = api._dir.path().to_owned();
    let home_png = Designs::new(&home, demo)
        .add_file("home.png", PNG, now())
        .expect("png");
    let link = Designs::new(&home, demo)
        .add_figma(
            "Checkout",
            design::parse_figma("https://www.figma.com/design/abc123/App?node-id=1-2")
                .expect("a figma link"),
            now(),
        )
        .expect("link");
    Designs::new(&home, other)
        .add_file("theirs.png", PNG, now())
        .expect("other png");
    let token = api.planner_token("DEMO").await;

    let (s, plan) = api
        .call_with(&token, Method::GET, "/v1/planner/plan", None)
        .await;
    assert_eq!(s, StatusCode::OK, "{plan}");
    assert!(plan["chains"].is_array(), "the plan is still there: {plan}");
    let designs = plan["designs"].as_array().expect("designs");
    assert_eq!(designs.len(), 2, "only this project's: {plan}");
    let file = home
        .join("designs")
        .join(demo.to_string())
        .join(format!("{}.png", home_png.id));
    assert_eq!(designs[0]["name"], "home.png");
    assert_eq!(designs[0]["kind"], "image");
    assert_eq!(designs[0]["mime"], "image/png");
    assert_eq!(designs[0]["path"], file.display().to_string());
    assert!(file.is_file(), "the path names a kept file");
    assert_eq!(designs[1]["name"], "Checkout");
    assert_eq!(
        designs[1]["path"],
        Value::Null,
        "a link without an image has no file"
    );
    assert_eq!(link.kind, DesignKind::Figma);

    // show_design: by id, and by name in any letter case; the metadata and the path.
    for reference in [home_png.id.to_string(), "HOME.PNG".to_owned()] {
        let (s, one) = api
            .call_with(
                &token,
                Method::GET,
                &format!("/v1/planner/designs/{reference}"),
                None,
            )
            .await;
        assert_eq!(s, StatusCode::OK, "{one}");
        assert_eq!(one["id"], home_png.id.to_string());
        assert_eq!(one["path"], file.display().to_string());
        assert_eq!(one["size"], PNG.len());
    }
    // Another project's design is not found, and a person's token is refused.
    let (s, e) = api
        .call_with(&token, Method::GET, "/v1/planner/designs/theirs.png", None)
        .await;
    assert_eq!(s, StatusCode::NOT_FOUND, "{e}");
    let (s, _) = api
        .call_with(
            super::TOKEN,
            Method::GET,
            "/v1/planner/designs/home.png",
            None,
        )
        .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
}
