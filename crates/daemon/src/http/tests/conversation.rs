//! A project's conversation with its assistant through the full router. The planner is a fake:
//! the test calls its own routes with its token, as its MCP tools do (`say`, `add_card`, ...); the
//! person's calls are the owner's routes.

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};

use super::Api;

const CONVERSATION: &str = "/v1/projects/DEMO/conversation";

async fn messages(api: &Api, query: &str) -> Vec<Value> {
    api.get(&format!("{CONVERSATION}{query}")).await["messages"]
        .as_array()
        .expect("messages")
        .clone()
}

fn gist(m: &Value) -> String {
    format!(
        "{}: {}",
        m["author"].as_str().unwrap_or("?"),
        m["text"].as_str().unwrap_or("?")
    )
}

#[tokio::test]
async fn a_person_message_and_the_assistants_say_are_stored_in_order_and_survive_a_restart() {
    let api = Api::new();
    api.two_projects().await;
    let planner = api.planner_token("DEMO").await;

    let said = api
        .post(
            CONVERSATION,
            json!({ "text": "  Plan a login page  ", "attachments": ["shot.png"] }),
        )
        .await;
    assert_eq!(said["author"], "person");
    assert_eq!(said["text"], "Plan a login page", "trimmed");
    assert_eq!(said["attachments"], json!(["shot.png"]));
    // The person's bubble is there before the planner has answered.
    assert_eq!(
        messages(&api, "")
            .await
            .iter()
            .map(gist)
            .collect::<Vec<_>>(),
        ["person: Plan a login page"]
    );

    let (status, answer) = api
        .call_with(
            &planner,
            Method::POST,
            "/v1/planner/say",
            Some(json!({ "text": "Which provider?" })),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{answer}");
    assert_eq!(answer["author"], "assistant");

    let api = api.restart();
    let all = messages(&api, "").await;
    assert_eq!(
        all.iter().map(gist).collect::<Vec<_>>(),
        ["person: Plan a login page", "assistant: Which provider?"],
        "both are kept, oldest first"
    );
    assert!(all[0]["at"].as_i64() <= all[1]["at"].as_i64());
    assert_eq!(
        api.get("/v1/projects/OTHER/conversation").await["messages"],
        json!([])
    );

    // Each message reached the stream as a journal entry of the project.
    let events = api.get("/v1/events?since=0").await;
    let kinds: Vec<_> = events
        .as_array()
        .expect("events")
        .iter()
        .filter(|e| e["entity"] == "conversation")
        .map(|e| {
            (
                e["kind"].as_str().unwrap_or(""),
                e["payload"]["message"]["text"].as_str().unwrap_or(""),
            )
        })
        .collect();
    assert_eq!(
        kinds,
        [
            ("message", "Plan a login page"),
            ("message", "Which provider?")
        ]
    );
}

#[tokio::test]
async fn say_is_the_planners_alone_and_stays_in_its_project_and_needs_words() {
    let api = Api::new();
    api.two_projects().await;
    let planner = api.planner_token("DEMO").await;
    let say = |token: String, body: Value| {
        let api = &api;
        async move {
            api.call_with(&token, Method::POST, "/v1/planner/say", Some(body))
                .await
        }
    };
    // A person, and a card's agent, are not the planner.
    let (s, _) = say(super::TOKEN.to_owned(), json!({ "text": "hi" })).await;
    assert_eq!(s, StatusCode::FORBIDDEN, "a person has no say tool");
    // A card of another project is another project's: 403, and nothing is stored.
    let (s, body) = say(planner.clone(), json!({ "text": "hi", "about": "OTHER-1" })).await;
    assert_eq!(s, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(body["code"], "forbidden");
    let (s, _) = say(
        planner.clone(),
        json!({ "text": "hi", "about": "nothing-called-this" }),
    )
    .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (s, _) = say(planner.clone(), json!({ "text": "  " })).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert!(messages(&api, "").await.is_empty());
    // The other project's planner reaches only its own conversation.
    api.post("/v1/projects/OTHER/conversation", json!({ "text": "mine" }))
        .await;
    assert!(messages(&api, "").await.is_empty());
    // A planner token does not read the conversation: that is the person's route.
    let (s, _) = api
        .call_with(&planner, Method::GET, CONVERSATION, None)
        .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, _) = api
        .call_with(
            &planner,
            Method::POST,
            CONVERSATION,
            Some(json!({ "text": "x" })),
        )
        .await;
    assert_eq!(
        s,
        StatusCode::FORBIDDEN,
        "the planner does not write as the person"
    );
}

#[tokio::test]
async fn what_the_assistant_did_is_a_line_of_toknilas_that_links_to_it() {
    let api = Api::new();
    api.two_projects().await;
    api.post("/v1/chains/plan/tickets", json!({ "title": "First" }))
        .await;
    let planner = api.planner_token("DEMO").await;
    let call = |m: Method, uri: &'static str, body: Value| {
        let (api, planner) = (&api, planner.clone());
        async move {
            let (s, v) = api.call_with(&planner, m, uri, Some(body)).await;
            assert!(s.is_success(), "{uri}: {s} {v}");
        }
    };
    call(
        Method::POST,
        "/v1/planner/chains/plan/cards",
        json!({ "title": "Second" }),
    )
    .await;
    call(
        Method::PATCH,
        "/v1/planner/cards/DEMO-2",
        json!({ "prompt": "Do it" }),
    )
    .await;
    call(
        Method::POST,
        "/v1/planner/cards/DEMO-2/threads",
        json!({ "kind": "design", "title": "Look" }),
    )
    .await;
    call(
        Method::POST,
        "/v1/planner/suggestion",
        json!({ "text": "Start DEMO-1", "card": "DEMO-1", "action": "start" }),
    )
    .await;
    let lines: Vec<(String, Value)> = messages(&api, "")
        .await
        .into_iter()
        .map(|m| (gist(&m), m["about"].clone()))
        .collect();
    let card = |k: &str| json!({ "kind": "card", "key": k });
    assert_eq!(
        lines,
        [
            ("toknila: added card DEMO-2".to_owned(), card("DEMO-2")),
            ("toknila: edited DEMO-2".to_owned(), card("DEMO-2")),
            (
                "toknila: opened thread 1 on DEMO-2".to_owned(),
                card("DEMO-2")
            ),
            (
                "toknila: suggested: Start DEMO-1".to_owned(),
                card("DEMO-1")
            ),
        ]
    );
}

#[tokio::test]
async fn a_card_discussion_is_in_the_conversation_with_its_card_and_is_stored_once() {
    let api = Api::new();
    api.two_projects().await;
    api.post("/v1/chains/plan/tickets", json!({ "title": "Card" }))
        .await;
    let planner = api.planner_token("DEMO").await;
    api.post(CONVERSATION, json!({ "text": "General question" }))
        .await;
    api.post(
        "/v1/tickets/DEMO-1/discussion",
        json!({ "text": "Why this card?" }),
    )
    .await;
    let (s, v) = api
        .call_with(
            &planner,
            Method::POST,
            "/v1/planner/cards/DEMO-1/threads/1/messages",
            Some(json!({ "text": "Because." })),
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    api.call_with(
        &planner,
        Method::POST,
        "/v1/planner/say",
        Some(json!({ "text": "Done", "about": "DEMO-1" })),
    )
    .await;

    let all = messages(&api, "").await;
    assert_eq!(
        all.iter().map(gist).collect::<Vec<_>>(),
        [
            "person: General question",
            "person: Why this card?",
            "assistant: Because.",
            "assistant: Done"
        ]
    );
    let card = json!({ "kind": "card", "key": "DEMO-1" });
    assert_eq!(all[0]["about"], Value::Null);
    assert_eq!(
        (&all[1]["about"], &all[2]["about"], &all[3]["about"]),
        (&card, &card, &card)
    );
    assert_eq!(
        (&all[1]["thread"], &all[2]["thread"], &all[3]["thread"]),
        (&json!(1), &json!(1), &Value::Null)
    );

    // The same messages, with the same ids, are the card's thread: one copy.
    let thread = api.get("/v1/tickets/DEMO-1/threads").await["threads"][0].clone();
    let ids: Vec<&Value> = thread["messages"]
        .as_array()
        .expect("messages")
        .iter()
        .map(|m| &m["id"])
        .collect();
    assert_eq!(ids, [&all[1]["id"], &all[2]["id"]]);
    // A discussion of another project's card never shows here.
    api.post(
        "/v1/tickets/OTHER-1/discussion",
        json!({ "text": "not yours" }),
    )
    .await;
    assert_eq!(messages(&api, "").await.len(), 4);
}

#[tokio::test]
async fn messages_with_one_timestamp_keep_the_order_they_were_said_in() {
    let api = Api::new();
    api.two_projects().await;
    api.post("/v1/chains/plan/tickets", json!({ "title": "Card" }))
        .await;
    let planner = api.planner_token("DEMO").await;
    // Stored, thread, stored, thread: interleaved across the two tables.
    api.post(CONVERSATION, json!({ "text": "one" })).await;
    api.post("/v1/tickets/DEMO-1/discussion", json!({ "text": "two" }))
        .await;
    api.post(CONVERSATION, json!({ "text": "three" })).await;
    let (s, v) = api
        .call_with(
            &planner,
            Method::POST,
            "/v1/planner/cards/DEMO-1/threads/1/messages",
            Some(json!({ "text": "four" })),
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    // Every message at the very same millisecond, as on a fast or coarse clock.
    let db = rusqlite::Connection::open(api._dir.path().join("t.db")).expect("db");
    db.execute_batch(
        "UPDATE conversation_messages SET created_at = 1000; UPDATE thread_messages SET created_at = 1000;",
    )
    .expect("pin the time");
    let want = [
        "person: one",
        "person: two",
        "person: three",
        "assistant: four",
    ];
    let all = messages(&api, "").await;
    assert_eq!(all.iter().map(gist).collect::<Vec<_>>(), want);
    // A page cut in the middle of the tie holds the same order.
    let page = messages(
        &api,
        &format!("?before={}&limit=2", all[3]["id"].as_str().expect("id")),
    )
    .await;
    assert_eq!(page.iter().map(gist).collect::<Vec<_>>(), &want[1..3]);
}

#[tokio::test]
async fn the_conversation_pages_by_before_newest_page_first_and_nothing_twice() {
    let api = Api::new();
    api.two_projects().await;
    api.post("/v1/chains/plan/tickets", json!({ "title": "Card" }))
        .await;
    for n in 1..=7 {
        // Some in the card's thread, some in the project's own table.
        if n % 3 == 0 {
            api.post(
                "/v1/tickets/DEMO-1/discussion",
                json!({ "text": format!("m{n}") }),
            )
            .await;
        } else {
            api.post(CONVERSATION, json!({ "text": format!("m{n}") }))
                .await;
        }
        tokio::time::sleep(std::time::Duration::from_millis(3)).await;
    }
    let newest = api.get(&format!("{CONVERSATION}?limit=3")).await;
    let text = |p: &Value| -> Vec<String> {
        p["messages"]
            .as_array()
            .expect("messages")
            .iter()
            .map(|m| m["text"].as_str().unwrap_or("").to_owned())
            .collect()
    };
    assert_eq!(text(&newest), ["m5", "m6", "m7"]);
    assert_eq!(newest["more"], true);
    let first = newest["messages"][0]["id"].as_str().expect("id");
    let second = api
        .get(&format!("{CONVERSATION}?limit=3&before={first}"))
        .await;
    assert_eq!(text(&second), ["m2", "m3", "m4"]);
    assert_eq!(second["more"], true);
    let first = second["messages"][0]["id"].as_str().expect("id");
    let last = api
        .get(&format!("{CONVERSATION}?limit=3&before={first}"))
        .await;
    assert_eq!(text(&last), ["m1"]);
    assert_eq!(last["more"], false, "nothing older");
    // The default page holds all seven; a before nobody has is an empty page.
    assert_eq!(text(&api.get(CONVERSATION).await).len(), 7);
    let none = api
        .get(&format!("{CONVERSATION}?before=01JBQ7ZJ0000000000000000ZZ"))
        .await;
    assert_eq!((text(&none).len(), &none["more"]), (0, &json!(false)));
    let (s, _) = api
        .call(Method::GET, "/v1/projects/NOPE/conversation", None)
        .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn deleting_the_project_takes_its_conversation_with_it() {
    let api = Api::new();
    api.two_projects().await;
    api.post(CONVERSATION, json!({ "text": "bye" })).await;
    api.post(
        "/v1/projects/OTHER/conversation",
        json!({ "text": "stays" }),
    )
    .await;
    api.post("/v1/projects/DEMO/delete", json!({})).await;
    let (s, _) = api.call(Method::GET, CONVERSATION, None).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let left = api.get("/v1/projects/OTHER/conversation").await;
    assert_eq!(
        left["messages"].as_array().map(Vec::len),
        Some(1),
        "the other project keeps its own"
    );
    // Its journal entries went too: only the other project's message is left.
    let events = api.get("/v1/events?since=0").await;
    let texts: Vec<_> = events
        .as_array()
        .expect("events")
        .iter()
        .filter(|e| e["entity"] == "conversation")
        .map(|e| e["payload"]["message"]["text"].as_str().unwrap_or(""))
        .collect();
    assert_eq!(texts, ["stays"]);
}

#[tokio::test]
async fn a_person_message_about_a_chain_is_stored_with_it_and_other_contexts_are_refused() {
    let api = Api::new();
    api.two_projects().await;
    api.post("/v1/chains/plan/tickets", json!({ "title": "First" }))
        .await;
    let said = api
        .post(
            CONVERSATION,
            json!({ "text": "reorder", "about": { "kind": "chain", "name": "plan" } }),
        )
        .await;
    assert_eq!(said["about"], json!({ "kind": "chain", "name": "plan" }));
    let (s, _) = api
        .call(
            Method::POST,
            CONVERSATION,
            Some(json!({ "text": "x", "about": { "kind": "chain", "name": "nope" } })),
        )
        .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (s, _) = api
        .call(
            Method::POST,
            CONVERSATION,
            Some(json!({ "text": "x", "about": { "kind": "card", "key": "DEMO-1" } })),
        )
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
}
