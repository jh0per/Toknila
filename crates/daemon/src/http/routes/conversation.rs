//! A project's conversation with its assistant: the person reads it and writes in it, the
//! assistant answers with `say` (a planner route, `PLANNER_ROUTES`).

use crate::http::body::Json;
use axum::Extension;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use toknila_api_types::{ConversationPage, ConversationQuery, NewPersonMessage, Say};
use toknila_app::{conversation, resolve};
use toknila_core::Actor;
use toknila_core::conversation::{About, Message};

use super::{ApiResult, read};
use crate::http::error::ErrorBody;
use crate::server::{AppState, Principal, now};

#[utoipa::path(get, path = "/projects/{project}/conversation", tag = "projects", params(
        ("project" = String, Path, description = "Project id or key"), ConversationQuery),
    responses((status = 200, body = ConversationPage), (status = 404, body = ErrorBody)))]
/// The project's conversation with its assistant, a page at a time: the newest messages first
/// asked for, older ones with `before`. A page is oldest first; `more` says there are older ones.
/// A card's discussion is in it, as messages about the card.
pub async fn get_conversation(
    State(state): State<AppState>,
    Path(project): Path<String>,
    Query(q): Query<ConversationQuery>,
) -> ApiResult<Json<ConversationPage>> {
    let page = read(&state, move |repo| {
        Ok(conversation::page(
            repo,
            &project,
            q.before.as_deref(),
            q.limit,
        )?)
    })
    .await?;
    Ok(Json(page))
}

#[utoipa::path(post, path = "/projects/{project}/conversation", tag = "projects", request_body = NewPersonMessage,
    params(("project" = String, Path, description = "Project id or key")),
    responses((status = 201, body = Message), (status = 400, body = ErrorBody), (status = 404, body = ErrorBody)))]
/// A person says something to the project's assistant: the message is stored (and shown) first.
/// Typing it into the assistant's terminal is the caller's next step.
pub async fn post_conversation(
    State(state): State<AppState>,
    Path(project): Path<String>,
    Json(input): Json<NewPersonMessage>,
) -> ApiResult<(StatusCode, Json<Message>)> {
    let (m, _) = state
        .store
        .write(Actor::Human, now(), move |tx| -> ApiResult<Message> {
            let p = resolve::project(&*tx, &project)?;
            Ok(conversation::say_as_person(tx, &p, input)?)
        })
        .await?;
    Ok((StatusCode::CREATED, Json(m)))
}

#[utoipa::path(post, path = "/planner/say", tag = "planner", request_body = Say,
    responses((status = 201, body = Message), (status = 400, body = ErrorBody), (status = 403, body = ErrorBody), (status = 404, body = ErrorBody)))]
/// The planner says something to the person: an answer, a question, a report. The person reads
/// it in the chat of the assistant's screen; `about` names the card or chain it is about.
pub async fn planner_say(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Json(input): Json<Say>,
) -> ApiResult<(StatusCode, Json<Message>)> {
    let project = principal.planner_project()?;
    let (m, _) = state
        .store
        .write(Actor::Agent, now(), move |tx| -> ApiResult<Message> {
            let m = conversation::say_as_assistant(tx, project, input)?;
            // Words about a story also land in the story's own chat, as the assistant's bubble.
            if let Some(About::Card { key }) = &m.about {
                let t = resolve::ticket(&*tx, &key.to_string())?;
                super::asks::record_answer(tx, &t, &m.text)?;
            }
            Ok(m)
        })
        .await?;
    Ok((StatusCode::CREATED, Json(m)))
}
