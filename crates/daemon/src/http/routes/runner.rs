//! The runner's HTTP routes: the scrollback of a ticket's terminal and typing into it. The live
//! output is the WebSocket `/tickets/{ticket}/runner/stream` (`crate::runner::stream`).

use axum::Json;
use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use toknila_api_types::TerminalSize;
use toknila_api_types::runner::{HEADER_END, HEADER_OFFSET, HEADER_RUN, RunnerQuery};
use toknila_app::{resolve, runs};
use toknila_core::{Actor, AgentRole};

use super::ApiResult;
use crate::http::error::{ApiError, ErrorBody};
use crate::server::AppState;

fn header(name: &'static str, value: impl ToString) -> (HeaderName, HeaderValue) {
    let value = HeaderValue::from_str(&value.to_string()).unwrap_or(HeaderValue::from_static(""));
    (HeaderName::from_static(name), value)
}

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
        .await
        .map_err(|e| ApiError(toknila_app::AppError::internal(e.to_string())))?
        .ok_or_else(|| ApiError::not_found(format!("output of {}", t.key)))?;
    let headers = HeaderMap::from_iter([
        header(HEADER_RUN, chunk.run),
        header(HEADER_OFFSET, chunk.offset),
        header(HEADER_END, chunk.end),
    ]);
    Ok((headers, chunk.bytes).into_response())
}

#[utoipa::path(post, path = "/tickets/{ticket}/runner/input", tag = "runner",
    params(("ticket" = String, Path, description = "Ticket id or key")),
    request_body(content = Vec<u8>, content_type = "application/octet-stream", description = "Bytes typed into the agent's terminal, as they are"),
    responses((status = 204, description = "Written"), (status = 400, body = ErrorBody), (status = 404, body = ErrorBody), (status = 409, body = ErrorBody)))]
pub async fn runner_input(
    State(state): State<AppState>,
    Path(ticket): Path<String>,
    body: Bytes,
) -> ApiResult<StatusCode> {
    let t = state
        .store
        .read(move |repo| Ok(resolve::ticket(repo, &ticket)))
        .await??;
    // A person's action: this route is for people, agents use the event contract.
    runs::send_input(&*state.runners, Actor::Human, &t, &body)?;
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
