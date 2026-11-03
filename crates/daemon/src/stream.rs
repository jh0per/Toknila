//! `GET /v1/stream?since=<seq>`: WebSocket of journal events. First everything after `since`
//! from the journal, then live events, without gaps or duplicates.

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::response::Response;
use serde::Deserialize;
use tokio::sync::broadcast::error::RecvError;

use crate::server::AppState;
use crate::store::{EventRecord, Store};

/// Query of the stream.
#[derive(Debug, Default, Deserialize)]
pub struct StreamQuery {
    #[serde(default)]
    pub since: i64,
}

const PAGE: usize = 1000;

pub async fn stream(
    State(state): State<AppState>,
    Query(q): Query<StreamQuery>,
    ws: WebSocketUpgrade,
) -> Response {
    ws.on_upgrade(move |socket| async move {
        if let Err(e) = serve(socket, state.store, q.since).await {
            tracing::debug!(error = %e, "stream closed");
        }
    })
}

async fn send(socket: &mut WebSocket, e: &EventRecord) -> Result<(), String> {
    let text = serde_json::to_string(e).map_err(|e| e.to_string())?;
    socket
        .send(Message::Text(text.into()))
        .await
        .map_err(|e| e.to_string())
}

/// Sends journal entries after `last` until caught up. Returns the new last seq.
async fn backfill(socket: &mut WebSocket, store: &Store, mut last: i64) -> Result<i64, String> {
    loop {
        let page = store
            .events_since(last, PAGE)
            .await
            .map_err(|e| e.to_string())?;
        let n = page.len();
        for e in page {
            send(socket, &e).await?;
            last = e.seq;
        }
        if n < PAGE {
            return Ok(last);
        }
    }
}

async fn serve(mut socket: WebSocket, store: Store, since: i64) -> Result<(), String> {
    // Subscribe before reading the journal, so nothing committed in between is lost;
    // the seq check below drops what the backfill already sent.
    let mut live = store.subscribe();
    let mut last = backfill(&mut socket, &store, since).await?;
    loop {
        tokio::select! {
            msg = live.recv() => match msg {
                Ok(e) if e.seq > last => {
                    if e.seq > last + 1 {
                        // A gap means events committed between subscribe and backfill pages;
                        // read them from the journal instead.
                        last = backfill(&mut socket, &store, last).await?;
                        if e.seq <= last { continue; }
                    }
                    send(&mut socket, &e).await?;
                    last = e.seq;
                }
                Ok(_) => {}
                Err(RecvError::Lagged(_)) => last = backfill(&mut socket, &store, last).await?,
                Err(RecvError::Closed) => return Ok(()),
            },
            incoming = socket.recv() => match incoming {
                None | Some(Err(_)) | Some(Ok(Message::Close(_))) => return Ok(()),
                Some(Ok(_)) => {}
            },
        }
    }
}

#[cfg(test)]
mod tests;
