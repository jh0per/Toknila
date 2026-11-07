//! The reviewer's runner seen from the host: what its screen shows and whether it sits idle at its
//! prompt, for the case a reviewer ends a turn without recording a verdict. The Stop hook is the
//! signal when the profile has hooks; idle output is the fallback when it has none. Core decides
//! what follows (`TicketEvent::ReviewerStopped`).

use std::sync::Arc;

use toknila_app::runner::RunnerState;
use toknila_core::question::{SCREEN_LINES, last_lines};
use toknila_core::{AgentRole, Nudge, TicketId};

use super::Runners;

impl Runners {
    /// The last lines of the reviewer's screen, ANSI removed.
    pub fn reviewer_screen(&self, ticket: TicketId) -> Vec<String> {
        last_lines(&self.tail(ticket, AgentRole::Reviewer), SCREEN_LINES)
    }

    /// What the live reviewer of the ticket is like; `None` without one.
    pub fn reviewer_state(&self, ticket: TicketId) -> Option<RunnerState> {
        let active = self.active().get(&(ticket, AgentRole::Reviewer)).cloned();
        active?.state()
    }

    /// The nudge settings of the profile the ticket's live reviewer was started with.
    pub fn reviewer_nudge(&self, ticket: TicketId) -> Option<Nudge> {
        let profile = self
            .active()
            .get(&(ticket, AgentRole::Reviewer))?
            .run()
            .profile
            .clone();
        self.profile(&profile).ok().map(|p| p.nudge)
    }

    /// The tickets whose reviewer is idle at its prompt and cannot tell us by a hook: no Stop
    /// hook is wired, nothing runs under it, and it printed nothing for the profile's `idle_secs`.
    pub fn idle_reviewers(self: &Arc<Self>) -> Vec<TicketId> {
        self.live_in(AgentRole::Reviewer)
            .into_iter()
            .filter(|t| {
                let (Some(state), Some(cfg)) = (self.reviewer_state(*t), self.reviewer_nudge(*t))
                else {
                    return false;
                };
                !state.hooks && !state.busy && state.quiet_ms >= cfg.idle_secs.saturating_mul(1000)
            })
            .collect()
    }
}
