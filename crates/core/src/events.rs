//! Domain events: every change a command makes, as typed data. The daemon journals each one
//! (`entity`, `entity_id`, `kind`, and the event's fields as `payload`) in the same transaction
//! as the state, and streams it to clients. The wire shape is pinned by tests below.

use serde::Serialize;

use crate::{
    Chain, Comment, Edge, Project, Release, ReleaseState, StatusReason, Subtask, Ticket, TicketKey,
    TicketStatus,
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
    /// `subtask`. Edge events belong to their chain.
    pub fn entity(&self) -> &'static str {
        match self {
            Self::ProjectCreated { .. } => "project",
            Self::ChainCreated { .. } | Self::EdgeAdded { .. } | Self::EdgeRemoved { .. } => {
                "chain"
            }
            Self::TicketCreated { .. }
            | Self::TicketStatusChanged { .. }
            | Self::TicketUpdated { .. } => "ticket",
            Self::ReleaseCreated { .. } | Self::ReleaseStateChanged { .. } => "release",
            Self::CommentAdded { .. } | Self::CommentDelivered { .. } => "comment",
            Self::SubtaskAdded { .. } | Self::SubtaskUpdated { .. } => "subtask",
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
            | Self::TicketUpdated { ticket } => ticket.id.to_string(),
            Self::ReleaseCreated { release } | Self::ReleaseStateChanged { release, .. } => {
                release.id.to_string()
            }
            Self::CommentAdded { comment, .. } | Self::CommentDelivered { comment, .. } => {
                comment.id.to_string()
            }
            Self::SubtaskAdded { subtask, .. } | Self::SubtaskUpdated { subtask, .. } => {
                subtask.id.to_string()
            }
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
        }
    }
}

#[cfg(test)]
mod tests;
