//! The inbox of a project and the person's decisions about leftovers.

use axum::Json;
use axum::extract::{Path, State};
use toknila_app::leftovers::{self, LeftoverAction};
use toknila_app::{inbox, resolve};
use toknila_core::Actor;
use toknila_core::inbox::InboxItem;
use toknila_core::suggestion::Suggestion;

use super::{ApiResult, read, ticket_write};
use crate::http::dto::TicketDetail;
use crate::http::error::ErrorBody;
use crate::server::{AppState, now};

#[utoipa::path(get, path = "/projects/{project}/inbox", tag = "projects",
    params(("project" = String, Path, description = "Project id or key")),
    responses((status = 200, body = Vec<InboxItem>), (status = 404, body = ErrorBody)))]
/// Everything in the project that waits on a person, newest first, as core computes it.
pub async fn get_inbox(
    State(state): State<AppState>,
    Path(project): Path<String>,
) -> ApiResult<Json<Vec<InboxItem>>> {
    // The live runs are read first, outside the store: their output is not the store's business.
    let live = state.runners.facts(now());
    let items = read(&state, move |repo| {
        let p = resolve::project(repo, &project)?;
        Ok(inbox::load_with(repo, p.id, &live)?)
    })
    .await?;
    Ok(Json(items))
}

#[utoipa::path(post, path = "/tickets/{ticket}/leftovers/{action}", tag = "tickets",
    params(
        ("ticket" = String, Path, description = "The earlier card that holds the open subtasks (id or key)"),
        ("action" = String, Path, description = "move (into a new card at the end of the chain), close or leave"),
    ),
    responses((status = 200, body = TicketDetail), (status = 404, body = ErrorBody), (status = 409, body = ErrorBody)))]
/// A person's decision about the open subtasks left behind a card done out of order.
pub async fn resolve_leftovers(
    State(state): State<AppState>,
    Path((ticket, action)): Path<(String, String)>,
) -> ApiResult<Json<TicketDetail>> {
    let action = action.parse::<LeftoverAction>()?;
    ticket_write(&state, ticket, Actor::Human, move |tx, t| {
        Ok(leftovers::resolve(tx, &t, action)?)
    })
    .await
}

#[utoipa::path(get, path = "/projects/{project}/suggestion", tag = "projects",
    params(("project" = String, Path, description = "Project id or key")),
    responses((status = 200, body = Option<Suggestion>), (status = 404, body = ErrorBody)))]
/// The planner's standing suggestion for the project (the next step), or `null` when it has none.
pub async fn get_suggestion(
    State(state): State<AppState>,
    Path(project): Path<String>,
) -> ApiResult<Json<Option<Suggestion>>> {
    let s = read(&state, move |repo| {
        let p = resolve::project(repo, &project)?;
        Ok(repo.suggestion(p.id)?)
    })
    .await?;
    Ok(Json(s))
}
