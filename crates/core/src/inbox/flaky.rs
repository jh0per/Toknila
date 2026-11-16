//! The `flaky` item: a check failed once and passed on the second run, so the card went on, and
//! the person learns of the test that failed. It needs nothing from the person to go on; they
//! may make a card of it, or dismiss it.

use super::{InboxItem, InboxKind, item};
use crate::Ticket;

/// The item of `t`'s last check when it passed only on its second run and nobody dismissed it.
pub(super) fn of_card(t: &Ticket) -> Option<InboxItem> {
    let check = t.review.last_check.as_ref().filter(|c| c.flaky)?;
    if t.review.flaky_dismissed {
        return None;
    }
    let text = format!(
        "A test failed once and passed on the second run while checking {}: {}",
        t.key,
        check.first_failing_line()
    );
    let mut i = item(InboxKind::Flaky, t, check.at, text);
    i.detail = Some(check.flaky_output.clone());
    // Nothing the card's own actions could do: the item is read, then dismissed.
    i.allowed_actions = Vec::new();
    i.needs_human = false;
    Some(i)
}

/// Why a flaky item cannot be dismissed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FlakyError {
    #[error("{0} has no flaky test to dismiss")]
    NothingToDismiss(crate::TicketKey),
}

/// Checks that `t` has an undismissed flaky item.
pub fn require_pending(t: &Ticket) -> Result<(), FlakyError> {
    of_card(t)
        .map(|_| ())
        .ok_or_else(|| FlakyError::NothingToDismiss(t.key.clone()))
}
