//! Small guards the transition table shares.

use super::TransitionError;
use crate::Actor;

/// Wildcard decisions belong to a person; an agent or the system cannot make them.
pub fn person_only(by: Actor, event: &'static str) -> Result<(), TransitionError> {
    if by == Actor::Human {
        Ok(())
    } else {
        Err(TransitionError::PersonOnly { event, actor: by })
    }
}

pub(super) fn non_empty(text: &str, what: &'static str) -> Result<(), TransitionError> {
    if text.trim().is_empty() {
        Err(TransitionError::EmptyText(what))
    } else {
        Ok(())
    }
}
