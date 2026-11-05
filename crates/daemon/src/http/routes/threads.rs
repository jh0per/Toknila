//! `/tickets/{ticket}/threads`: threads on a card. Every route here runs as a person; an agent
//! reaches the same use cases through `/tickets/{ticket}/events`, where approval does not exist.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use toknila_app::{resolve, threads};
use toknila_core::{Actor, ThreadId, Ticket};

use super::{ApiResult, load_thread_detail, read};
use crate::http::dto::{NewMessage, NewThread, NewVersion, ThreadDetail, TicketThreads};
use crate::http::error::ErrorBody;
use crate::server::{AppState, now};
use crate::store::WriteTx;

const TICKET: &str = "Ticket id or key, e.g. DEMO-3";
const THREAD: &str = "Thread number on the ticket (1, 2, ...) or thread id";

#[utoipa::path(get, path = "/tickets/{ticket}/threads", tag = "threads",
    params(("ticket" = String, Path, description = TICKET)),
    responses((status = 200, body = TicketThreads), (status = 404, body = ErrorBody)))]
/// A ticket's threads with their messages and versions, and its phase.
pub async fn list_threads(
    State(state): State<AppState>,
    Path(ticket): Path<String>,
) -> ApiResult<Json<TicketThreads>> {
    Ok(Json(
        read(&state, move |repo| {
            let t = resolve::ticket(repo, &ticket)?;
            let all = repo.threads(t.id)?;
            let phase = toknila_core::thread::phase(&all);
            let threads = all
                .into_iter()
                .map(|th| load_thread_detail(repo, th))
                .collect::<ApiResult<Vec<_>>>()?;
            Ok(TicketThreads {
                ticket: t.key.to_string(),
                phase,
                threads,
            })
        })
        .await?,
    ))
}

#[utoipa::path(post, path = "/tickets/{ticket}/threads", tag = "threads", request_body = NewThread,
    params(("ticket" = String, Path, description = TICKET)),
    responses((status = 201, body = ThreadDetail), (status = 400, body = ErrorBody), (status = 404, body = ErrorBody)))]
/// A person opens a thread on a ticket.
pub async fn open_thread(
    State(state): State<AppState>,
    Path(ticket): Path<String>,
    Json(input): Json<NewThread>,
) -> ApiResult<(StatusCode, Json<ThreadDetail>)> {
    let detail = thread_write(&state, ticket, move |tx, t| {
        Ok(threads::open(tx, t, input.kind, &input.title)?.id)
    })
    .await?;
    Ok((StatusCode::CREATED, detail))
}

#[utoipa::path(post, path = "/tickets/{ticket}/threads/{thread}/messages", tag = "threads", request_body = NewMessage,
    params(("ticket" = String, Path, description = TICKET), ("thread" = String, Path, description = THREAD)),
    responses((status = 200, body = ThreadDetail), (status = 400, body = ErrorBody), (status = 404, body = ErrorBody)))]
/// A person posts a message in a thread.
pub async fn post_thread_message(
    State(state): State<AppState>,
    Path((ticket, thread)): Path<(String, String)>,
    Json(input): Json<NewMessage>,
) -> ApiResult<Json<ThreadDetail>> {
    thread_write(&state, ticket, move |tx, t| {
        let th = threads::find(&*tx, t, &thread)?;
        threads::post_message(tx, t, &th, &input.text)?;
        Ok(th.id)
    })
    .await
}

#[utoipa::path(post, path = "/tickets/{ticket}/threads/{thread}/versions", tag = "threads", request_body = NewVersion,
    params(("ticket" = String, Path, description = TICKET), ("thread" = String, Path, description = THREAD)),
    responses((status = 200, body = ThreadDetail), (status = 400, body = ErrorBody), (status = 404, body = ErrorBody)))]
/// A person posts a new artifact version; an approved thread reopens.
pub async fn post_thread_version(
    State(state): State<AppState>,
    Path((ticket, thread)): Path<(String, String)>,
    Json(input): Json<NewVersion>,
) -> ApiResult<Json<ThreadDetail>> {
    thread_write(&state, ticket, move |tx, t| {
        let th = threads::find(&*tx, t, &thread)?;
        threads::post_version(tx, t, &th, input)?;
        Ok(th.id)
    })
    .await
}

#[utoipa::path(post, path = "/tickets/{ticket}/threads/{thread}/approve", tag = "threads",
    params(("ticket" = String, Path, description = TICKET), ("thread" = String, Path, description = THREAD)),
    responses((status = 200, body = ThreadDetail), (status = 404, body = ErrorBody), (status = 409, body = ErrorBody)))]
/// A person approves a thread; its latest version is pinned.
pub async fn approve_thread(
    State(state): State<AppState>,
    Path((ticket, thread)): Path<(String, String)>,
) -> ApiResult<Json<ThreadDetail>> {
    thread_write(&state, ticket, move |tx, t| {
        let th = threads::find(&*tx, t, &thread)?;
        Ok(threads::approve(tx, t, &th)?.id)
    })
    .await
}

#[utoipa::path(post, path = "/tickets/{ticket}/threads/{thread}/changes", tag = "threads", request_body = NewMessage,
    params(("ticket" = String, Path, description = TICKET), ("thread" = String, Path, description = THREAD)),
    responses((status = 200, body = ThreadDetail), (status = 400, body = ErrorBody), (status = 404, body = ErrorBody)))]
/// A person asks for changes: a message that keeps the thread open, or reopens it.
pub async fn request_thread_changes(
    State(state): State<AppState>,
    Path((ticket, thread)): Path<(String, String)>,
    Json(input): Json<NewMessage>,
) -> ApiResult<Json<ThreadDetail>> {
    thread_write(&state, ticket, move |tx, t| {
        let th = threads::find(&*tx, t, &thread)?;
        threads::request_changes(tx, t, &th, &input.text)?;
        Ok(th.id)
    })
    .await
}

/// Runs `f` as a person's command on a ticket and answers with the fresh detail of the thread
/// it returns.
async fn thread_write(
    state: &AppState,
    ticket: String,
    f: impl FnOnce(&mut WriteTx<'_>, &Ticket) -> ApiResult<ThreadId> + Send + 'static,
) -> ApiResult<Json<ThreadDetail>> {
    let (detail, _) = state
        .store
        .write(Actor::Human, now(), move |tx| -> ApiResult<ThreadDetail> {
            let t = resolve::ticket(&*tx, &ticket)?;
            let id = f(tx, &t)?;
            let fresh = threads::find(&*tx, &t, &id.to_string())?;
            load_thread_detail(&tx.repo(), fresh)
        })
        .await?;
    Ok(Json(detail))
}
