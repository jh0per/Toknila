//! Domain events: every change a command makes, as typed data. The daemon journals each one
//! (`entity`, `entity_id`, `kind`, and the event's fields as `payload`) in the same transaction
//! as the state, and streams it to clients. The wire shape is pinned by tests below.

use serde::Serialize;

use crate::{
    Actor, ArtifactFormat, Chain, Comment, Edge, Project, Release, ReleaseState, Ship,
    StatusReason, Subtask, Thread, ThreadId, ThreadMessage, Ticket, TicketKey, TicketStatus,
};

/// One change. Serialises as its payload: the variant's fields, without a tag.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum DomainEvent {
    ProjectCreated {
        project: Project,
    },
    ChainCreated {
        chain: Chain,
    },
    EdgeAdded {
        edge: Edge,
    },
    EdgeRemoved {
        edge: Edge,
    },
    TicketCreated {
        ticket: Ticket,
    },
    TicketStatusChanged {
        key: TicketKey,
        from: TicketStatus,
        to: TicketStatus,
        reason: Option<StatusReason>,
        ticket: Ticket,
    },
    TicketUpdated {
        ticket: Ticket,
    },
    /// A wildcard card was proposed: a new ticket in Proposed, outside the plan.
    TicketProposed {
        ticket: Ticket,
    },
    /// A person let a wildcard card into the plan.
    TicketApproved {
        key: TicketKey,
        ticket: Ticket,
    },
    /// A person turned a wildcard card down.
    TicketRejected {
        key: TicketKey,
        reason: String,
        ticket: Ticket,
    },
    /// The ticket's ship (worktree and branch) was made.
    ShipCreated {
        ship: Ship,
    },
    /// The ticket's worktree was removed; its branch stays.
    ShipRemoved {
        ship: Ship,
    },
    ReleaseCreated {
        release: Release,
    },
    ReleaseStateChanged {
        version: String,
        from: ReleaseState,
        to: ReleaseState,
        release: Release,
    },
    /// `ticket` is the ticket's label (its key).
    CommentAdded {
        ticket: String,
        comment: Comment,
    },
    /// `ticket` is the ticket's label (its key).
    CommentDelivered {
        ticket: String,
        comment: Comment,
    },
    SubtaskAdded {
        ticket: TicketKey,
        subtask: Subtask,
    },
    SubtaskUpdated {
        ticket: TicketKey,
        subtask: Subtask,
    },
    /// A thread was opened on a ticket.
    ThreadOpened {
        ticket: TicketKey,
        thread: Thread,
    },
    ThreadMessagePosted {
        ticket: TicketKey,
        message: ThreadMessage,
    },
    /// A new artifact version; the body stays in the database, the journal carries its size.
    ThreadVersionPosted {
        ticket: TicketKey,
        thread: ThreadId,
        number: u32,
        format: ArtifactFormat,
        author: Actor,
        bytes: usize,
    },
    /// A person approved a thread.
    ThreadApproved {
        ticket: TicketKey,
        thread: Thread,
    },
    /// An approved thread went back to open (a new version, or a request for changes).
    ThreadReopened {
        ticket: TicketKey,
        thread: Thread,
    },
}

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

    /// Kind of entity the event is about: `project`, `chain`, `ticket`, `release`, `comment`,
    /// `subtask`, `thread`. Edge events belong to their chain.
    pub fn entity(&self) -> &'static str {
        match self {
            Self::ProjectCreated { .. } => "project",
            Self::ChainCreated { .. } | Self::EdgeAdded { .. } | Self::EdgeRemoved { .. } => {
                "chain"
            }
            Self::TicketCreated { .. }
            | Self::TicketStatusChanged { .. }
            | Self::TicketUpdated { .. }
            | Self::TicketProposed { .. }
            | Self::TicketApproved { .. }
            | Self::TicketRejected { .. }
            | Self::ShipCreated { .. }
            | Self::ShipRemoved { .. } => "ticket",
            Self::ReleaseCreated { .. } | Self::ReleaseStateChanged { .. } => "release",
            Self::CommentAdded { .. } | Self::CommentDelivered { .. } => "comment",
            Self::SubtaskAdded { .. } | Self::SubtaskUpdated { .. } => "subtask",
            Self::ThreadOpened { .. }
            | Self::ThreadMessagePosted { .. }
            | Self::ThreadVersionPosted { .. }
            | Self::ThreadApproved { .. }
            | Self::ThreadReopened { .. } => "thread",
        }
    }

    /// Id of the entity the event is about.
    pub fn entity_id(&self) -> String {
        match self {
            Self::ProjectCreated { project } => project.id.to_string(),
            Self::ChainCreated { chain } => chain.id.to_string(),
            Self::EdgeAdded { edge } | Self::EdgeRemoved { edge } => edge.chain_id.to_string(),
            Self::TicketCreated { ticket }
            | Self::TicketStatusChanged { ticket, .. }
            | Self::TicketUpdated { ticket }
            | Self::TicketProposed { ticket }
            | Self::TicketApproved { ticket, .. }
            | Self::TicketRejected { ticket, .. } => ticket.id.to_string(),
            Self::ShipCreated { ship } | Self::ShipRemoved { ship } => ship.ticket_id.to_string(),
            Self::ReleaseCreated { release } | Self::ReleaseStateChanged { release, .. } => {
                release.id.to_string()
            }
            Self::CommentAdded { comment, .. } | Self::CommentDelivered { comment, .. } => {
                comment.id.to_string()
            }
            Self::SubtaskAdded { subtask, .. } | Self::SubtaskUpdated { subtask, .. } => {
                subtask.id.to_string()
            }
            Self::ThreadOpened { thread, .. }
            | Self::ThreadApproved { thread, .. }
            | Self::ThreadReopened { thread, .. } => thread.id.to_string(),
            Self::ThreadMessagePosted { message, .. } => message.thread_id.to_string(),
            Self::ThreadVersionPosted { thread, .. } => thread.to_string(),
        }
    }

    /// What happened to the entity.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::ProjectCreated { .. }
            | Self::ChainCreated { .. }
            | Self::TicketCreated { .. }
            | Self::ReleaseCreated { .. } => "created",
            Self::EdgeAdded { .. } => "edge_added",
            Self::EdgeRemoved { .. } => "edge_removed",
            Self::TicketStatusChanged { .. } => "status_changed",
            Self::TicketUpdated { .. } | Self::SubtaskUpdated { .. } => "updated",
            Self::ReleaseStateChanged { .. } => "state_changed",
            Self::CommentAdded { .. } | Self::SubtaskAdded { .. } => "added",
            Self::CommentDelivered { .. } => "delivered",
            Self::ShipCreated { .. } => "ship_created",
            Self::ShipRemoved { .. } => "ship_removed",
            Self::TicketProposed { .. } => "proposed",
            Self::TicketApproved { .. } => "approved",
            Self::TicketRejected { .. } => "rejected",
            Self::ThreadOpened { .. } => "opened",
            Self::ThreadMessagePosted { .. } => "message_posted",
            Self::ThreadVersionPosted { .. } => "version_posted",
            Self::ThreadApproved { .. } => "approved",
            Self::ThreadReopened { .. } => "reopened",
        }
    }
}

#[cfg(test)]
mod tests;
