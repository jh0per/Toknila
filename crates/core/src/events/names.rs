//! Where an event goes in the journal: its entity and the entity's id (the kind of change is
//! `DomainEvent::kind`, beside the enum).

use super::DomainEvent;

impl DomainEvent {
    /// Kind of entity the event is about: `project`, `chain`, `ticket`, `release`, `comment`,
    /// `subtask`, `thread`. Edge events belong to their chain.
    pub fn entity(&self) -> &'static str {
        match self {
            Self::ProjectCreated { .. }
            | Self::ProjectUpdated { .. }
            | Self::ProjectDeleted { .. } => "project",
            Self::ChainCreated { .. }
            | Self::ChainUpdated { .. }
            | Self::ChainDeleted { .. }
            | Self::EdgeAdded { .. }
            | Self::EdgeRemoved { .. }
            | Self::EdgeMoved { .. } => "chain",
            Self::DesignMoved { .. }
            | Self::TicketMoved { .. }
            | Self::TicketCreated { .. }
            | Self::TicketStatusChanged { .. }
            | Self::TicketUpdated { .. }
            | Self::TicketProposed { .. }
            | Self::TicketApproved { .. }
            | Self::TicketRejected { .. }
            | Self::ShipCreated { .. }
            | Self::ShipRemoved { .. }
            | Self::ShipRemoveFailed { .. }
            | Self::RunnerStarted { .. }
            | Self::RunnerExited { .. }
            | Self::QuestionRaised { .. }
            | Self::QuestionCleared { .. }
            | Self::QuestionAnswered { .. } => "ticket",
            Self::ReleaseCreated { .. } | Self::ReleaseStateChanged { .. } => "release",
            Self::CommentAdded { .. } | Self::CommentDelivered { .. } => "comment",
            Self::SubtaskAdded { .. } | Self::SubtaskUpdated { .. } => "subtask",
            Self::ThreadOpened { .. }
            | Self::ThreadMessagePosted { .. }
            | Self::ThreadVersionPosted { .. }
            | Self::ThreadApproved { .. }
            | Self::ThreadReopened { .. }
            | Self::ThreadWithdrawn { .. } => "thread",
            Self::DesignPinAsked { .. }
            | Self::DesignPinAnswered { .. }
            | Self::DesignPinRemoved { .. } => "design_pin",
            Self::WindowShowCard { .. }
            | Self::WindowSuggest { .. }
            | Self::WindowSuggestionCleared { .. } => "window",
            Self::ConversationMessage { .. } => "conversation",
            Self::PlannerTurn { .. } => "planner",
        }
    }

    /// Id of the entity the event is about.
    pub fn entity_id(&self) -> String {
        match self {
            Self::ProjectCreated { project }
            | Self::ProjectUpdated { project }
            | Self::ProjectDeleted { project } => project.id.to_string(),
            Self::ChainCreated { chain }
            | Self::ChainUpdated { chain }
            | Self::ChainDeleted { chain } => chain.id.to_string(),
            Self::EdgeAdded { edge }
            | Self::EdgeRemoved { edge }
            | Self::EdgeMoved { edge, .. } => edge.chain_id.to_string(),
            Self::DesignMoved { ticket, .. }
            | Self::TicketMoved { ticket, .. }
            | Self::TicketCreated { ticket }
            | Self::TicketStatusChanged { ticket, .. }
            | Self::TicketUpdated { ticket }
            | Self::TicketProposed { ticket }
            | Self::TicketApproved { ticket, .. }
            | Self::TicketRejected { ticket, .. } => ticket.id.to_string(),
            Self::ShipCreated { ship }
            | Self::ShipRemoved { ship }
            | Self::ShipRemoveFailed { ship, .. } => ship.ticket_id.to_string(),
            Self::RunnerStarted { run } | Self::RunnerExited { run, .. } => {
                run.ticket_id.to_string()
            }
            Self::QuestionRaised { ticket, .. }
            | Self::QuestionCleared { ticket, .. }
            | Self::QuestionAnswered { ticket, .. } => ticket.to_string(),
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
            | Self::ThreadReopened { thread, .. }
            | Self::ThreadWithdrawn { thread, .. } => thread.id.to_string(),
            Self::ThreadMessagePosted { message, .. } => message.thread_id.to_string(),
            Self::ThreadVersionPosted { thread, .. } => thread.to_string(),
            Self::DesignPinAsked { pin }
            | Self::DesignPinAnswered { pin }
            | Self::DesignPinRemoved { pin } => pin.id.to_string(),
            Self::WindowShowCard { project, .. }
            | Self::WindowSuggest { project, .. }
            | Self::WindowSuggestionCleared { project, .. }
            | Self::ConversationMessage { project, .. }
            | Self::PlannerTurn { project, .. } => project.to_string(),
        }
    }
}
