//! Constructors that derive an event from a before/after pair of an entity.

use super::DomainEvent;
use crate::{Release, StatusReason, Ticket, TicketStatus};

impl DomainEvent {
    /// A ticket changed: `status_changed` when its status moved, `updated` otherwise.
    pub fn ticket_changed(before: &Ticket, after: &Ticket) -> Self {
        if before.status == after.status {
            Self::TicketUpdated {
                ticket: after.clone(),
            }
        } else {
            Self::TicketStatusChanged {
                key: after.key.clone(),
                from: before.status,
                to: after.status,
                reason: after.status_reason.clone(),
                ticket: after.clone(),
            }
        }
    }

    /// A person's decision on a wildcard card, when `before -> after` is one: it left Proposed.
    pub fn wildcard_decided(before: &Ticket, after: &Ticket) -> Option<Self> {
        if before.status != TicketStatus::Proposed || after.status == TicketStatus::Proposed {
            return None;
        }
        let ticket = after.clone();
        Some(match &after.status_reason {
            Some(StatusReason::Cancelled { text }) => Self::TicketRejected {
                key: after.key.clone(),
                reason: text.clone(),
                ticket,
            },
            _ => Self::TicketApproved {
                key: after.key.clone(),
                ticket,
            },
        })
    }

    /// A milestone changed (its state, cut or release time).
    pub fn release_changed(before: &Release, after: &Release) -> Self {
        Self::ReleaseStateChanged {
            version: after.version.clone(),
            from: before.state,
            to: after.state,
            release: after.clone(),
        }
    }
}
