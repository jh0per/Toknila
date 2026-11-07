//! What the host tells the inbox about the live runs: open permission dialogs, and for each run
//! its spawn, first sign of life, the end of its output and the last answer by keys.

use std::collections::HashSet;

use toknila_core::question::{self, RunFacts, ScreenAnswer};
use toknila_core::{AgentRole, TicketId, Timestamp};

use super::Runners;

/// How much of the end of a run's output the inbox reads to show its screen.
const TAIL_BYTES: usize = 16 * 1024;

impl Runners {
    /// What the host knows of the live runs for the inbox at `now`.
    pub fn facts(&self, now: Timestamp) -> toknila_app::inbox::Live {
        // A run whose process is known to be over asks nothing any more.
        let over: HashSet<(TicketId, AgentRole)> = self
            .active()
            .iter()
            .filter(|(_, a)| a.is_over())
            .map(|(key, _)| *key)
            .collect();
        let permissions = self
            .prompts()
            .pending()
            .into_iter()
            .filter(|p| !over.contains(&(p.ticket, p.role)))
            .collect();
        let runs = self
            .tokens()
            .spawn_facts()
            .into_iter()
            .filter(|s| !over.contains(&(s.ticket, s.role)))
            .map(|s| RunFacts {
                ticket: s.ticket,
                role: s.role,
                spawned: s.spawned,
                first_activity: s.first_activity,
                startup_secs: s.startup_secs,
                tail: self.tail(s.ticket, s.role),
                answered: s.answered,
            })
            .collect();
        toknila_app::inbox::Live {
            now: Some(now),
            permissions,
            runs,
        }
    }

    /// The raw end of the run's output; empty without a live run.
    pub fn tail(&self, ticket: TicketId, role: AgentRole) -> Vec<u8> {
        let active = self.active().get(&(ticket, role)).cloned();
        active.map(|a| a.tail(TAIL_BYTES)).unwrap_or_default()
    }

    /// The answer a person gives to the run's screen at `at`: the hash of the screen it is given
    /// to, kept so the item stays quiet until the screen changes (or the grace passes).
    pub fn screen_answer(&self, ticket: TicketId, role: AgentRole, at: Timestamp) -> ScreenAnswer {
        ScreenAnswer {
            at,
            screen: question::screen_hash(&self.tail(ticket, role)),
        }
    }
}
