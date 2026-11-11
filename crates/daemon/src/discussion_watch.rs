//! A discussion message that gets no answer: after the wait the daemon logs a warning. The window
//! offers the person a nudge (the planner's own input route) at the same moment.

use std::time::Duration;

use toknila_core::discussion::{self, ANSWER_WAIT_SECS};
use toknila_core::{ThreadMessageId, TicketId};

use crate::store::Store;

/// Waits `wait`, then says whether `message` of the card's discussion is still the unanswered
/// newest one; when it is, logs a warning. Returns whether it warned.
pub async fn warn_if_unanswered(
    store: &Store,
    ticket: TicketId,
    key: &str,
    message: ThreadMessageId,
    wait: Duration,
) -> bool {
    tokio::time::sleep(wait).await;
    let found = store
        .read(move |repo| {
            let mut out = false;
            for th in repo.threads(ticket)? {
                let all = repo.thread_messages(th.id)?;
                out |= discussion::unanswered(&all).is_some_and(|m| m.id == message);
            }
            Ok(out)
        })
        .await;
    let unanswered = found.unwrap_or(false);
    if unanswered {
        tracing::warn!(card = key, message = %message, secs = wait.as_secs(), "a discussion message got no answer from the assistant");
    }
    unanswered
}

/// Starts the watch for a message just said, with the standard wait.
pub fn watch(store: Store, ticket: TicketId, key: String, message: ThreadMessageId) {
    tokio::spawn(async move {
        warn_if_unanswered(
            &store,
            ticket,
            &key,
            message,
            Duration::from_secs(ANSWER_WAIT_SECS),
        )
        .await;
    });
}
