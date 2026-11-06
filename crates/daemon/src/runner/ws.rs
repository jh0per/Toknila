//! `GET /v1/tickets/{ticket}/runner/stream?run=<id>&from=<offset>`: the ticket's terminal output
//! as it is written. Binary frames carry raw bytes, a text frame says the run ended (the formats
//! are in `toknila_api_types::runner`). Without `run` and `from` it starts at the first byte the
//! scrollback still holds; with them, right after the bytes a client already has. `role=reviewer`
//! follows the reviewer's terminal instead of the worker's; both are read-only here.

use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, Query, State};
use axum::response::Response;
use serde::Deserialize;
use tokio::sync::broadcast::error::RecvError;
use toknila_api_types::runner::{RunnerControl, encode_frame};
use toknila_app::resolve;
use toknila_core::{AgentRole, RunId, TicketId};

use super::{Chunk, Runners};
use crate::http::error::ApiError;
use crate::server::AppState;

/// Longest payload of one frame.
const FRAME: usize = 64 * 1024;

/// Query of the stream.
#[derive(Debug, Default, Deserialize)]
pub struct StreamQuery {
    /// The run the client already has output of, with `from` the offset after its last byte.
    pub run: Option<RunId>,
    pub from: Option<u64>,
    /// `worker` (the default) or `reviewer`.
    pub role: Option<AgentRole>,
}

/// Upgrades to the runner stream of one ticket.
pub async fn stream(
    State(state): State<AppState>,
    Path(ticket): Path<String>,
    Query(q): Query<StreamQuery>,
    ws: WebSocketUpgrade,
) -> Result<Response, ApiError> {
    let ticket = state
        .store
        .read(move |repo| Ok(resolve::ticket(repo, &ticket)))
        .await??
        .id;
    let cursor = q.run.zip(q.from);
    let role = q.role.unwrap_or(AgentRole::Worker);
    Ok(ws.on_upgrade(move |socket| async move {
        if let Err(e) = serve(socket, state.runners, (ticket, role), cursor).await {
            tracing::debug!(error = %e, "runner stream closed");
        }
    }))
}

async fn send(socket: &mut WebSocket, message: Message) -> Result<(), String> {
    socket.send(message).await.map_err(|e| e.to_string())
}

async fn read(
    runners: &Arc<Runners>,
    (ticket, role): (TicketId, AgentRole),
    cursor: Option<(RunId, u64)>,
) -> Option<Chunk> {
    let runners = runners.clone();
    tokio::task::spawn_blocking(move || runners.read(ticket, role, cursor))
        .await
        .ok()
        .flatten()
}

async fn serve(
    mut socket: WebSocket,
    runners: Arc<Runners>,
    which: (TicketId, AgentRole),
    mut cursor: Option<(RunId, u64)>,
) -> Result<(), String> {
    let ticket = which.0;
    // Subscribe before reading, so output written in between wakes the loop again.
    let mut wake = runners.subscribe();
    let mut told_ended: Option<RunId> = None;
    loop {
        if let Some(chunk) = read(&runners, which, cursor).await {
            let mut at = chunk.offset;
            for part in chunk.bytes.chunks(FRAME) {
                send(
                    &mut socket,
                    Message::Binary(encode_frame(chunk.run, at, part).into()),
                )
                .await?;
                at += part.len() as u64;
            }
            cursor = Some((chunk.run, chunk.end));
            if chunk.ended && told_ended != Some(chunk.run) {
                told_ended = Some(chunk.run);
                let control = RunnerControl::Ended {
                    run: chunk.run,
                    exit: chunk.exit,
                };
                let text = serde_json::to_string(&control).map_err(|e| e.to_string())?;
                send(&mut socket, Message::Text(text.into())).await?;
            }
        }
        loop {
            tokio::select! {
                woken = wake.recv() => match woken {
                    Ok(t) if t != ticket => continue,
                    Ok(_) | Err(RecvError::Lagged(_)) => break,
                    Err(RecvError::Closed) => return Ok(()),
                },
                incoming = socket.recv() => match incoming {
                    None | Some(Err(_) | Ok(Message::Close(_))) => return Ok(()),
                    Some(Ok(_)) => {}
                },
            }
        }
    }
}
