//! The silence ticker: every few seconds it looks at each live run (`toknila_app::nudge`), and
//! carries out what the rule decided in one command, so a nudge is delivered after the commit like
//! any instruction and giving up moves the card through the machine.

use std::sync::Arc;
use std::time::Duration;

use tokio::task::JoinHandle;
use toknila_app::{AppError, nudge};
use toknila_core::nudge::Decision;
use toknila_core::{Actor, Timestamp};

use super::Runners;
use crate::server::now;
use crate::store::Store;

/// How often live runs are looked at.
const EVERY: Duration = Duration::from_secs(5);

/// How often live runs are looked at: [`EVERY`], or the milliseconds in `TOKNILA_TICK_MS` when
/// `TOKNILA_TEST_ROOT` is set too (a test daemon; a real one ignores the variable).
fn every() -> Duration {
    if std::env::var_os("TOKNILA_TEST_ROOT").is_none() {
        return EVERY;
    }
    std::env::var("TOKNILA_TICK_MS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .filter(|ms| *ms > 0)
        .map_or(EVERY, Duration::from_millis)
}

/// Evaluates every live run until the daemon ends.
pub fn watch(store: Store, runners: Arc<Runners>) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut timer = tokio::time::interval(every());
        loop {
            timer.tick().await;
            tick(&store, &runners, now()).await;
        }
    })
}

/// One round at time `at`: the runs that are quiet are nudged or given up on.
pub async fn tick(store: &Store, runners: &Arc<Runners>, at: Timestamp) {
    super::screens::tick(store, runners, at).await;
    super::pausing::tick(store, runners, at).await;
    for ticket in runners.idle_reviewers() {
        let screen = runners.reviewer_screen(ticket);
        let done = store
            .write(Actor::System, at, move |tx| -> Result<(), AppError> {
                toknila_app::reviews::stopped(tx, ticket, screen)
            })
            .await;
        if let Err(error) = done {
            tracing::warn!(%ticket, %error, "the idle reviewer check failed");
        }
    }
    for ticket in runners.live() {
        if let Err(error) = look(store, runners, ticket, at).await {
            tracing::warn!(%ticket, %error, "the silence check failed");
        }
    }
}

async fn look(
    store: &Store,
    runners: &Arc<Runners>,
    ticket: toknila_core::TicketId,
    at: Timestamp,
) -> Result<(), String> {
    let (Some(cfg), Some(mut reported)) =
        (runners.nudge_of(ticket), runners.tokens().reported(ticket))
    else {
        return Ok(());
    };
    // An open screen question is a person's: the rule never types into it.
    reported.screen_open = runners
        .prompts()
        .screen_flagged(ticket, toknila_core::AgentRole::Worker);
    // The process list is read here, outside the store, so the writer is never held up by it.
    let live = toknila_app::Runner::state(runners.as_ref(), ticket);
    let cfg_read = cfg.clone();
    let decision = store
        .read(move |repo| Ok(nudge::check(repo, live, ticket, &cfg_read, reported, at)))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    if decision == Decision::Wait {
        return Ok(());
    }
    store
        .write(Actor::System, at, move |tx| -> Result<(), AppError> {
            nudge::apply(tx, ticket, &cfg, decision)
        })
        .await
        .map(|_| ())
        .map_err(|e: AppError| e.to_string())
}
