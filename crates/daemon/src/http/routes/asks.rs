//! A person asks the project's assistant about a story (`/tickets/{ticket}/ask`), and the
//! assistant's answer is kept as a comment of the story (`record_answer`, used by `say`).
//! Asking changes nothing on the story and queues no instruction.

use crate::http::body::Json;
use axum::extract::{Path, State};
use toknila_api_types::{AskReply, AskStory};
use toknila_app::{AppError, AppResult, Reader, UnitOfWork, notes, resolve};
use toknila_core::instructions::ask_line;
use toknila_core::{Actor, CommentKind, Ticket};

use super::ApiResult;
use crate::http::error::ErrorBody;
use crate::server::{AppState, now};

#[utoipa::path(post, path = "/tickets/{ticket}/ask", tag = "tickets", request_body = AskStory,
    params(("ticket" = String, Path, description = "Ticket id or key")),
    responses((status = 201, body = AskReply), (status = 400, body = ErrorBody), (status = 404, body = ErrorBody)))]
/// A person asks the project's assistant a question about the story. The question is kept in the
/// story's chat (a comment that is never an instruction); the answer carries the line to type
/// into the assistant's terminal: the question with the story's record. The assistant answers
/// with `say` about the story; the answer lands in the same chat.
pub async fn ask_story(
    State(state): State<AppState>,
    Path(ticket): Path<String>,
    Json(input): Json<AskStory>,
) -> ApiResult<(axum::http::StatusCode, Json<AskReply>)> {
    let text = input.text.trim().to_owned();
    if text.is_empty() {
        return Err(AppError::bad_request("a question needs some text").into());
    }
    let (reply, _) = state
        .store
        .write(Actor::Human, now(), move |tx| -> ApiResult<AskReply> {
            let t = resolve::ticket(&*tx, &ticket)?;
            let view = tx
                .chain_view(t.chain_id)?
                .ok_or_else(|| AppError::not_found("chain"))?;
            let comment = notes::add_comment(
                tx,
                &view,
                t.id,
                Actor::Human,
                CommentKind::Question,
                notes::body_from_text(&text),
            )?;
            // The story as it is after the question was written down.
            let fresh = tx.ticket(t.id)?.unwrap_or(t);
            let line = ask_line(
                &fresh,
                &tx.subtasks(fresh.id)?,
                &tx.comments(fresh.id)?,
                &text,
            );
            Ok(AskReply { comment, line })
        })
        .await?;
    Ok((axum::http::StatusCode::CREATED, Json(reply)))
}

/// The assistant said something about a story: it also lands as the assistant's comment in the
/// story's chat, where the person asked. Only a comment: the story itself is not touched.
pub fn record_answer<U: UnitOfWork + ?Sized>(uow: &mut U, t: &Ticket, text: &str) -> AppResult<()> {
    let view = uow
        .chain_view(t.chain_id)?
        .ok_or_else(|| AppError::not_found("chain"))?;
    notes::add_comment(
        uow,
        &view,
        t.id,
        Actor::Assistant,
        CommentKind::Report,
        notes::body_from_text(text),
    )?;
    Ok(())
}
