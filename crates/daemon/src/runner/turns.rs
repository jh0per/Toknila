//! The turn of each project's live planner, in memory only (the rules are
//! `toknila_core::planner::turn`): whether it works on a message, its step and since when. A turn
//! belongs to one run, so a run that ended, the next run and a daemon that started have none on.
//! Nothing here is stored; the start and the end of a turn are journaled by the caller.

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, PoisonError};

use toknila_app::AppError;
use toknila_core::planner::turn::{Signal, Turn};
use toknila_core::{Actor, ProjectId, RunId, TicketId, Timestamp, planner};

use super::Runners;
use crate::server::now;
use crate::store::Store;

/// The planners' turns, by run key, each with the run it belongs to.
#[derive(Debug, Default)]
pub struct Turns(Mutex<HashMap<TicketId, (RunId, Turn)>>);

impl Turns {
    fn map(&self) -> MutexGuard<'_, HashMap<TicketId, (RunId, Turn)>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Applies `signal` of the planner run `run` (keyed `key`) that came at `at`, with `step` as
    /// its line; `Some(working)` when the turn started or ended.
    pub fn apply(
        &self,
        key: TicketId,
        run: RunId,
        signal: Signal,
        step: Option<String>,
        at: Timestamp,
    ) -> Option<bool> {
        let mut map = self.map();
        let (owner, turn) = map.entry(key).or_insert_with(|| (run, Turn::default()));
        if *owner != run {
            // Another run's turn is that run's: this one starts from none.
            (*owner, *turn) = (run, Turn::default());
        }
        let (next, change) = turn.after(signal, step, at);
        *turn = next;
        change
    }

    /// The turn of the run `run` of the planner keyed `key`; another run's turn is none.
    pub fn of(&self, key: TicketId, run: RunId) -> Turn {
        self.map()
            .get(&key)
            .filter(|(owner, _)| *owner == run)
            .map(|(_, turn)| turn.clone())
            .unwrap_or_default()
    }

    /// Forgets the turn of the planner keyed `key` (only `run`'s, when given); whether one was on.
    pub fn end(&self, key: TicketId, run: Option<RunId>) -> bool {
        let mut map = self.map();
        let ours = map
            .get(&key)
            .is_some_and(|(owner, _)| run.is_none_or(|r| r == *owner));
        ours && map.remove(&key).is_some_and(|(_, turn)| turn.working)
    }
}

impl Runners {
    /// The turns of the projects' planners.
    pub fn turns(&self) -> &Turns {
        &self.turns
    }
}

/// Ends the turn of `project`'s planner (only `run`'s, when given) and, when one was on, journals
/// its end (`planner.turn`, not working): the person stopped the planner, or its run ended.
pub async fn end_planner_turn(
    store: &Store,
    runners: &Runners,
    project: ProjectId,
    run: Option<RunId>,
    actor: Actor,
) -> Result<(), AppError> {
    if runners.turns().end(planner::run_key(project), run) {
        store
            .write(actor, now(), move |tx| -> Result<(), AppError> {
                toknila_app::planner::turn(tx, project, false)
            })
            .await?;
    }
    Ok(())
}
