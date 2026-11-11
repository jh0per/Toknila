//! A card's discussion with the assistant through the full router. The planner is a fake: the
//! test calls the planner's own routes with its token, as its MCP tools do (`thread_message` for
//! the answer, `edit_card` for the proposal); the person's calls are the owner's routes.

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};

use super::Api;

struct Talk<'a> {
    api: &'a Api,
    planner: String,
}

impl Talk<'_> {
    /// A call the planner makes.
    async fn planner(&self, m: Method, uri: &str, body: Value) -> (StatusCode, Value) {
        self.api.call_with(&self.planner, m, uri, Some(body)).await
    }

    async fn thread(&self) -> Value {
        let all = self.api.get("/v1/tickets/DEMO-1/threads").await;
        all["threads"][0].clone()
    }

    /// The id of the newest message of `kind` in the discussion.
    async fn newest(&self, kind: &str) -> String {
        let t = self.thread().await;
        let found = t["messages"]
            .as_array()
            .expect("messages")
            .iter()
            .rfind(|m| m["kind"] == kind)
            .unwrap_or_else(|| panic!("no {kind} message in {t}"))
            .clone();
        found["id"].as_str().expect("id").to_owned()
    }

    async fn card(&self) -> Value {
        self.api.get("/v1/tickets/DEMO-1").await["ticket"].clone()
    }
}

async fn setup(api: &Api) -> Talk<'_> {
    api.two_projects().await;
    api.post(
        "/v1/chains/plan/tickets",
        json!({ "title": "Card", "prompt": "old prompt", "acceptance": ["one"] }),
    )
    .await;
    Talk {
        api,
        planner: api.planner_token("DEMO").await,
    }
}

const DECIDE: &str = "/v1/tickets/DEMO-1/threads/1/edits";

#[tokio::test]
async fn a_message_is_kept_in_a_discussion_thread_the_planner_answers_in_and_it_never_blocks() {
    let api = Api::new();
    let talk = setup(&api).await;

    let said = api
        .post(
            "/v1/tickets/DEMO-1/discussion",
            json!({ "text": "make the acceptance stricter" }),
        )
        .await;
    assert_eq!(said["thread"]["kind"], "discussion");
    assert_eq!(said["thread"]["number"], 1);
    assert_eq!(said["thread"]["state"], "approved");
    assert_eq!(said["messages"][0]["author"], "human");
    assert_eq!(said["messages"][0]["text"], "make the acceptance stricter");

    // The second message goes into the same thread.
    let again = api
        .post(
            "/v1/tickets/DEMO-1/discussion",
            json!({ "text": "and the prompt" }),
        )
        .await;
    assert_eq!(again["thread"]["id"], said["thread"]["id"]);
    assert_eq!(again["messages"].as_array().expect("messages").len(), 2);

    // The planner answers in the thread; the answer is the agent's.
    let (s, d) = talk
        .planner(
            Method::POST,
            "/v1/planner/cards/DEMO-1/threads/1/messages",
            json!({ "text": "I would add the 4xx case" }),
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{d}");
    let last = d["messages"].as_array().expect("messages").last().cloned();
    let last = last.expect("an answer");
    assert_eq!(
        (&last["author"], &last["text"]),
        (&json!("agent"), &json!("I would add the 4xx case"))
    );

    // It is not a design: nothing blocks, no phase of design.
    let all = api.get("/v1/tickets/DEMO-1/threads").await;
    assert_eq!(all["phase"], "implement");
}

#[tokio::test]
async fn an_edit_the_planner_proposes_lands_only_when_the_person_accepts_it() {
    let api = Api::new();
    let talk = setup(&api).await;
    api.post(
        "/v1/tickets/DEMO-1/discussion",
        json!({ "text": "rewrite it" }),
    )
    .await;

    let (s, _) = talk
        .planner(
            Method::PATCH,
            "/v1/planner/cards/DEMO-1",
            json!({ "prompt": "new prompt", "acceptance": ["one", "two"] }),
        )
        .await;
    assert_eq!(s, StatusCode::OK);
    let card = talk.card().await;
    assert_eq!(card["prompt"], "old prompt", "nothing landed yet");
    assert_eq!(card["acceptance"], json!(["one"]));

    let id = talk.newest("edit_proposal").await;
    let t = talk.thread().await;
    let proposal: Value = serde_json::from_str(
        t["messages"]
            .as_array()
            .expect("messages")
            .iter()
            .find(|m| m["id"] == id.as_str())
            .expect("proposal")["text"]
            .as_str()
            .expect("text"),
    )
    .expect("proposal json");
    assert_eq!(
        proposal,
        json!({
            "prompt": { "from": "old prompt", "to": "new prompt" },
            "acceptance": { "from": ["one"], "to": ["one", "two"] },
        })
    );

    // The planner cannot accept it: the route is a person's.
    let (s, body) = talk
        .planner(Method::POST, &format!("{DECIDE}/{id}/accept"), json!({}))
        .await;
    assert_eq!(s, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(talk.card().await["prompt"], "old prompt");

    let done = api.post(&format!("{DECIDE}/{id}/accept"), json!({})).await;
    assert_eq!(
        done["messages"]
            .as_array()
            .expect("messages")
            .last()
            .expect("last")["kind"],
        "edit_accepted"
    );
    let card = talk.card().await;
    assert_eq!(card["prompt"], "new prompt");
    assert_eq!(card["acceptance"], json!(["one", "two"]));

    // Deciding twice is a conflict, and changes nothing.
    let (s, _) = api
        .call(Method::POST, &format!("{DECIDE}/{id}/reject"), None)
        .await;
    assert_eq!(s, StatusCode::CONFLICT);
}

#[tokio::test]
async fn a_rejected_edit_leaves_the_card_as_it_was() {
    let api = Api::new();
    let talk = setup(&api).await;
    api.post(
        "/v1/tickets/DEMO-1/discussion",
        json!({ "text": "change it" }),
    )
    .await;
    talk.planner(
        Method::PATCH,
        "/v1/planner/cards/DEMO-1",
        json!({ "prompt": "another prompt" }),
    )
    .await;
    let id = talk.newest("edit_proposal").await;
    let done = api.post(&format!("{DECIDE}/{id}/reject"), json!({})).await;
    assert_eq!(
        done["messages"]
            .as_array()
            .expect("messages")
            .last()
            .expect("last")["kind"],
        "edit_rejected"
    );
    let card = talk.card().await;
    assert_eq!(card["prompt"], "old prompt");
    assert_eq!(card["acceptance"], json!(["one"]));
    let (s, _) = api
        .call(Method::POST, &format!("{DECIDE}/{id}/accept"), None)
        .await;
    assert_eq!(
        s,
        StatusCode::CONFLICT,
        "a rejected proposal cannot be accepted later"
    );
}

#[tokio::test]
async fn an_edit_proposed_against_a_card_that_changed_since_is_refused_on_accept() {
    let api = Api::new();
    let talk = setup(&api).await;
    api.post(
        "/v1/tickets/DEMO-1/discussion",
        json!({ "text": "change it" }),
    )
    .await;
    talk.planner(
        Method::PATCH,
        "/v1/planner/cards/DEMO-1",
        json!({ "prompt": "proposed" }),
    )
    .await;
    let id = talk.newest("edit_proposal").await;
    let (s, _) = api
        .call(
            Method::PATCH,
            "/v1/tickets/DEMO-1",
            Some(json!({ "prompt": "the person's own edit" })),
        )
        .await;
    assert_eq!(s, StatusCode::OK);
    let (s, err) = api
        .call(Method::POST, &format!("{DECIDE}/{id}/accept"), None)
        .await;
    assert_eq!(s, StatusCode::CONFLICT, "{err}");
    assert_eq!(talk.card().await["prompt"], "the person's own edit");
}

#[tokio::test]
async fn without_a_discussion_the_planners_edit_lands_at_once_as_before() {
    let api = Api::new();
    let talk = setup(&api).await;
    let (s, _) = talk
        .planner(
            Method::PATCH,
            "/v1/planner/cards/DEMO-1",
            json!({ "prompt": "direct" }),
        )
        .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(talk.card().await["prompt"], "direct");
}

/// The newest message of the card's discussion, as the watcher sees it.
async fn newest_human(api: &Api) -> (toknila_core::TicketId, toknila_core::ThreadMessageId) {
    let ticket = api.get("/v1/tickets/DEMO-1").await["ticket"]["id"]
        .as_str()
        .expect("id")
        .parse()
        .expect("ticket id");
    let all = api.get("/v1/tickets/DEMO-1/threads").await;
    let id = all["threads"][0]["messages"]
        .as_array()
        .expect("messages")
        .last()
        .expect("message")["id"]
        .as_str()
        .expect("id")
        .parse()
        .expect("message id");
    (ticket, id)
}

#[tokio::test]
async fn a_message_nobody_answers_in_time_is_logged_as_a_warning_and_an_answered_one_is_not() {
    let api = Api::new();
    let talk = setup(&api).await;
    api.post(
        "/v1/tickets/DEMO-1/discussion",
        json!({ "text": "is this right?" }),
    )
    .await;
    let (ticket, asked) = newest_human(&api).await;
    let wait = std::time::Duration::from_millis(20);
    // Control: unanswered for the whole wait, the watcher warns.
    assert!(
        crate::discussion_watch::warn_if_unanswered(&api.store, ticket, "DEMO-1", asked, wait)
            .await
    );
    // The assistant answers: the same watch finds nothing to warn about.
    talk.planner(
        Method::POST,
        "/v1/planner/cards/DEMO-1/threads/1/messages",
        json!({ "text": "yes" }),
    )
    .await;
    assert!(
        !crate::discussion_watch::warn_if_unanswered(&api.store, ticket, "DEMO-1", asked, wait)
            .await
    );
}

#[tokio::test]
async fn a_nudge_goes_the_planners_input_route_and_is_refused_when_the_planner_is_not_running() {
    let api = Api::new();
    let _talk = setup(&api).await;
    api.post("/v1/tickets/DEMO-1/discussion", json!({ "text": "hello?" }))
        .await;
    let (status, body) = api
        .call(
            Method::POST,
            "/v1/projects/DEMO/planner/input",
            Some(json!("Discussion on card DEMO-1 (thread 1): hello?")),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
}
