//! A run that was told to end is not one to reuse: a start waits for it to be gone.

use std::time::{Duration, Instant};

use toknila_app::runner::RunRequest;

use super::Runners;

/// How long a start waits for a run that was told to end to be gone.
const WAIT: Duration = Duration::from_secs(3);

/// Waits (bounded, with the map unlocked) until the run of the request's ticket and role, if it
/// was told to end, is gone, so the new run does not share its slot. When it is still there after
/// the bound it is replaced: its exit is reported by its own run id, so it cannot be taken for
/// the new run's end.
pub(super) fn wait_out(runners: &Runners, req: &RunRequest) {
    let dying = runners.active().get(&(req.ticket, req.role)).cloned();
    let Some(old) = dying.filter(|a| a.stopping() && !a.is_over()) else {
        return;
    };
    tracing::info!(ticket = %req.key, "a start found its run still ending; waiting for it");
    let until = Instant::now() + WAIT;
    while !old.is_over() {
        if Instant::now() >= until {
            tracing::info!(ticket = %req.key, "the old run is still ending; replacing it");
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}
