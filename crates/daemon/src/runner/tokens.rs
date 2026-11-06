//! Agent tokens: one per run, scoped to the run's ticket, created when the runner starts and
//! revoked when the run ends. The token reaches the agent only through the runner's environment.
//! The last sign of life of each ticket's agent is kept here too, for the nudge.

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, PoisonError};

use toknila_core::{RunId, TicketId, Timestamp};

use crate::server::constant_time_eq;

/// What an agent token allows: acting as the agent of this ticket, in this run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Grant {
    pub ticket: TicketId,
    pub run: RunId,
}

/// The last sign of life of a ticket's agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Activity {
    pub at: Timestamp,
    /// What it was: `start`, `api` for any call made with the token, or a hook (`hook:stop`).
    pub signal: String,
}

#[derive(Debug, Default)]
struct Inner {
    grants: Vec<(String, Grant)>,
    activity: HashMap<TicketId, Activity>,
}

/// The tokens of the live runs.
#[derive(Debug, Default)]
pub struct AgentTokens {
    inner: Mutex<Inner>,
}

impl AgentTokens {
    fn inner(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Issues a token for `grant` and starts its ticket's activity at `at`.
    pub fn issue(&self, grant: Grant, at: Timestamp) -> Result<String, String> {
        let token = crate::info::new_token()?;
        let mut inner = self.inner();
        inner.grants.push((token.clone(), grant));
        inner.activity.insert(
            grant.ticket,
            Activity {
                at,
                signal: "start".to_owned(),
            },
        );
        Ok(token)
    }

    /// The grant of `token`, if it is a live agent token.
    pub fn lookup(&self, token: &str) -> Option<Grant> {
        self.inner()
            .grants
            .iter()
            .find(|(t, _)| constant_time_eq(t.as_bytes(), token.as_bytes()))
            .map(|(_, g)| *g)
    }

    /// Revokes every token of `run`.
    pub fn revoke(&self, run: RunId) {
        self.inner().grants.retain(|(_, g)| g.run != run);
    }

    /// Records a sign of life from the agent of `ticket`.
    pub fn touch(&self, ticket: TicketId, at: Timestamp, signal: &str) {
        self.inner().activity.insert(
            ticket,
            Activity {
                at,
                signal: signal.to_owned(),
            },
        );
    }

    /// The last sign of life of the ticket's agent: its run's start, or later.
    pub fn last_activity(&self, ticket: TicketId) -> Option<Activity> {
        self.inner().activity.get(&ticket).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grant() -> Grant {
        Grant {
            ticket: TicketId::new(),
            run: RunId::new(),
        }
    }

    #[test]
    fn a_token_finds_its_grant_until_the_run_is_revoked() {
        let tokens = AgentTokens::default();
        let (a, b) = (grant(), grant());
        let ta = tokens.issue(a, Timestamp(1)).expect("token");
        let tb = tokens.issue(b, Timestamp(1)).expect("token");
        assert_ne!(ta, tb);
        assert_eq!(tokens.lookup(&ta), Some(a));
        assert_eq!(tokens.lookup(&tb), Some(b));
        tokens.revoke(a.run);
        assert_eq!(tokens.lookup(&ta), None);
        assert_eq!(tokens.lookup(&tb), Some(b));
        assert_eq!(tokens.lookup("nope"), None);
    }

    #[test]
    fn activity_starts_with_the_run_and_keeps_the_latest_sign() {
        let tokens = AgentTokens::default();
        let g = grant();
        assert_eq!(tokens.last_activity(g.ticket), None);
        tokens.issue(g, Timestamp(10)).expect("token");
        let first = tokens.last_activity(g.ticket).map(|a| a.at);
        assert_eq!(first, Some(Timestamp(10)));
        tokens.touch(g.ticket, Timestamp(25), "hook:stop");
        let last = tokens.last_activity(g.ticket).expect("activity");
        assert_eq!(
            (last.at, last.signal.as_str()),
            (Timestamp(25), "hook:stop")
        );
    }
}
