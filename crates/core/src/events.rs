//! Domain events: every change a command makes, as typed data. The daemon journals each one
//! (`entity`, `entity_id`, `kind`, and the event's fields as `payload`) in the same transaction
//! as the state, and streams it to clients. The wire shape is pinned by tests below.

use serde::Serialize;

use crate::design::pin::DesignPin;
use crate::question::{AnswerKind, QuestionKind};
use crate::{
    Actor, AgentRole, ArtifactFormat, Chain, Comment, Edge, ProcessExit, Project, ProjectId,
    ProjectKey, Release, ReleaseState, Run, Ship, StatusReason, Subtask, Thread, ThreadId,
    ThreadMessage, Ticket, TicketId, TicketKey, TicketStatus, suggestion::Suggestion,
};

/// One change. Serialises as its payload: the variant's fields, without a tag.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum DomainEvent {
    ProjectCreated {
        project: Project,
    },
    /// A project setting changed (the parallel agent limit).
    ProjectUpdated {
        project: Project,
    },
    /// A person deleted the project with everything in it; `project` is how it was. Its own
    /// journal entries and those of its chains, cards and milestones went with it.
    ProjectDeleted {
        project: Project,
    },
    ChainCreated {
        chain: Chain,
    },
    /// A chain's settings changed (its review mode or reviewer).
    ChainUpdated {
        chain: Chain,
    },
    /// The chain was deleted (alone, or with its project), cards and milestones included; `chain`
    /// is how it was.
    ChainDeleted {
        chain: Chain,
    },
    EdgeAdded {
        edge: Edge,
    },
    EdgeRemoved {
        edge: Edge,
    },
    /// A dependency moved to another end in one step: `edge` is the new one, `was` the old.
    EdgeMoved {
        edge: Edge,
        was: Edge,
    },
    /// A design reference moved from the card `from` to another in one step. `ticket` is the card
    /// that received it, as it is now.
    DesignMoved {
        design: crate::design::DesignRef,
        from: TicketKey,
        ticket: Ticket,
    },
    /// A card moved to another epic of its project. `ticket` is the card as it is now; `from` and
    /// `to` are the names of the epics.
    TicketMoved {
        from: String,
        to: String,
        ticket: Ticket,
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
    /// The worktree could not be removed (the ticket stays Merged); the path is still recorded.
    ShipRemoveFailed {
        ship: Ship,
        detail: String,
    },
    /// A runner process started for the ticket.
    RunnerStarted {
        run: Run,
    },
    /// The runner's process ended, on its own or because it was stopped.
    RunnerExited {
        run: Run,
        exit: ProcessExit,
    },
    /// A run of the ticket asked something that no status change shows: a reviewer's permission
    /// dialog, or an agent that waits on its screen.
    QuestionRaised {
        key: TicketKey,
        ticket: TicketId,
        role: AgentRole,
        kind: QuestionKind,
        /// In words, what is asked ("the agent's screen asks: <first line>"); absent for older
        /// entries.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        text: Option<String>,
    },
    /// Such a question went away without an answer: the agent showed life again, or its run ended.
    QuestionCleared {
        key: TicketKey,
        ticket: TicketId,
        role: AgentRole,
    },
    /// A person answered a run's question and the answer reached it. The journal entry's actor
    /// says who; `role` says whose run it was.
    QuestionAnswered {
        key: TicketKey,
        ticket: TicketId,
        role: AgentRole,
        answer: AnswerKind,
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
    /// An agent withdrew its own empty thread.
    ThreadWithdrawn {
        ticket: TicketKey,
        thread: Thread,
    },
    /// The assistant pinned a question on a design.
    DesignPinAsked {
        pin: DesignPin,
    },
    /// A person answered a pin (or changed the answer).
    DesignPinAnswered {
        pin: DesignPin,
    },
    /// An open pin went with the design it was made on.
    DesignPinRemoved {
        pin: DesignPin,
    },
    /// The project's assistant asked the window to show a card. Only a window that looks at that
    /// project follows it; nothing in the plan changes.
    WindowShowCard {
        project: ProjectId,
        project_key: ProjectKey,
        chain: String,
        ticket: TicketKey,
    },
    /// The project's assistant set its standing suggestion (the next step for the owner); it
    /// replaces the previous one.
    WindowSuggest {
        project: ProjectId,
        suggestion: Suggestion,
    },
    /// The standing suggestion ended: its action was done or its card moved on.
    WindowSuggestionCleared {
        project: ProjectId,
        project_key: ProjectKey,
    },
    /// A message joined the project's conversation with its assistant (the person's, the
    /// assistant's, or a line of Toknila's about what the assistant did).
    ConversationMessage {
        project: ProjectId,
        message: crate::conversation::Message,
    },
    /// The project's assistant took a message (`working`) or its turn ended (it stopped, or a
    /// person stopped it). What it does inside the turn is never journaled.
    PlannerTurn {
        project: ProjectId,
        working: bool,
    },
}

// `kind` stays in this file: the web app's test reads its arms to check that every kind has
// words (`apps/web/src/store/eventWords.test.ts`). `entity` and `entity_id` are in `names`.
impl DomainEvent {
    /// What happened to the entity.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::ProjectCreated { .. }
            | Self::ChainCreated { .. }
            | Self::TicketCreated { .. }
            | Self::ReleaseCreated { .. } => "created",
            Self::ProjectDeleted { .. } | Self::ChainDeleted { .. } => "deleted",
            Self::EdgeAdded { .. } => "edge_added",
            Self::EdgeRemoved { .. } => "edge_removed",
            Self::EdgeMoved { .. } => "edge_moved",
            Self::DesignMoved { .. } => "design_moved",
            Self::TicketMoved { .. } => "moved",
            Self::TicketStatusChanged { .. } => "status_changed",
            Self::TicketUpdated { .. }
            | Self::SubtaskUpdated { .. }
            | Self::ChainUpdated { .. }
            | Self::ProjectUpdated { .. } => "updated",
            Self::ReleaseStateChanged { .. } => "state_changed",
            Self::CommentAdded { .. } | Self::SubtaskAdded { .. } => "added",
            Self::CommentDelivered { .. } => "delivered",
            Self::ShipCreated { .. } => "ship_created",
            Self::ShipRemoved { .. } => "ship_removed",
            Self::ShipRemoveFailed { .. } => "ship_remove_failed",
            Self::RunnerStarted { .. } => "runner_started",
            Self::RunnerExited { .. } => "runner_exited",
            Self::QuestionRaised { .. } => "question_raised",
            Self::QuestionCleared { .. } => "question_cleared",
            Self::QuestionAnswered { .. } => "question_answered",
            Self::TicketProposed { .. } => "proposed",
            Self::TicketApproved { .. } => "approved",
            Self::TicketRejected { .. } => "rejected",
            Self::ThreadOpened { .. } => "opened",
            Self::ThreadMessagePosted { .. } => "message_posted",
            Self::ThreadVersionPosted { .. } => "version_posted",
            Self::ThreadApproved { .. } => "approved",
            Self::ThreadReopened { .. } => "reopened",
            Self::ThreadWithdrawn { .. } => "withdrawn",
            Self::DesignPinAsked { .. } => "asked",
            Self::DesignPinAnswered { .. } => "answered",
            Self::DesignPinRemoved { .. } => "removed",
            Self::WindowShowCard { .. } => "show_card",
            Self::WindowSuggest { .. } => "suggest",
            Self::WindowSuggestionCleared { .. } => "suggestion_cleared",
            Self::ConversationMessage { .. } => "message",
            Self::PlannerTurn { .. } => "turn",
        }
    }
}

mod constructors;
mod names;

#[cfg(test)]
mod tests;
