//! The runner's HTTP routes: the scrollback of a ticket's terminal and typing into it. The live
//! output is the WebSocket `/tickets/{ticket}/runner/stream` (`crate::runner::stream`).

use axum::Json;
use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::Response;
use toknila_api_types::TerminalSize;
use toknila_api_types::runner::RunnerQuery;
use toknila_app::{resolve, runs};
use toknila_core::{Actor, AgentRole};

use super::{ApiResult, scrollback_response};
use crate::http::error::ErrorBody;
use crate::server::AppState;
use toknila_app::AppError;

#[utoipa::path(get, path = "/tickets/{ticket}/runner/scrollback", tag = "runner",
    params(("ticket" = String, Path, description = "Ticket id or key"), RunnerQuery),
    responses(
        (status = 200, description = "Raw terminal output of the newest run of the worker (or, with role=reviewer, of the reviewer), last lines only (ANSI kept). Headers x-toknila-run, x-toknila-offset (of the first byte) and x-toknila-end (after the last; continue the stream from here).", body = Vec<u8>, content_type = "application/octet-stream"),
        (status = 404, body = ErrorBody)))]
pub async fn scrollback(
    State(state): State<AppState>,
    Path(ticket): Path<String>,
    Query(q): Query<RunnerQuery>,
) -> ApiResult<Response> {
    let role = q.role.unwrap_or(AgentRole::Worker);
    let t = state
        .store
        .read(move |repo| Ok(resolve::ticket(repo, &ticket)))
        .await??;
    let runners = state.runners.clone();
    let chunk = tokio::task::spawn_blocking(move || runners.read(t.id, role, None))
        .await?
        .ok_or_else(|| AppError::not_found(format!("output of {}", t.key)))?;
    Ok(scrollback_response(chunk))
}

#[utoipa::path(post, path = "/tickets/{ticket}/runner/input", tag = "runner",
    params(("ticket" = String, Path, description = "Ticket id or key"), RunnerQuery),
    request_body(content = Vec<u8>, content_type = "application/octet-stream", description = "Bytes typed into the agent's terminal, as they are"),
    responses((status = 204, description = "Written"), (status = 400, body = ErrorBody), (status = 404, body = ErrorBody), (status = 409, body = ErrorBody)))]
pub async fn runner_input(
    State(state): State<AppState>,
    Path(ticket): Path<String>,
    Query(q): Query<RunnerQuery>,
    body: Bytes,
) -> ApiResult<StatusCode> {
    let t = state
        .store
        .read(move |repo| Ok(resolve::ticket(repo, &ticket)))
        .await??;
    // A person's action: this route is for people, agents use the event contract.
    let role = q.role.unwrap_or(AgentRole::Worker);
    runs::send_input(&*state.runners, Actor::Human, &t, role, &body)?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(post, path = "/tickets/{ticket}/runner/size", tag = "runner",
    params(("ticket" = String, Path, description = "Ticket id or key")),
    request_body = TerminalSize,
    responses((status = 204, description = "Resized; sizes outside 20x5..500x200 are brought into that range"), (status = 404, body = ErrorBody), (status = 409, body = ErrorBody)))]
/// The size of the panel that shows the agent's terminal. The agent is a full-screen terminal
/// program, so its terminal follows the panel.
pub async fn runner_size(
    State(state): State<AppState>,
    Path(ticket): Path<String>,
    Json(size): Json<TerminalSize>,
) -> ApiResult<StatusCode> {
    let t = state
        .store
        .read(move |repo| Ok(resolve::ticket(repo, &ticket)))
        .await??;
    runs::resize(&*state.runners, Actor::Human, &t, size.cols, size.rows)?;
    Ok(StatusCode::NO_CONTENT)
}
