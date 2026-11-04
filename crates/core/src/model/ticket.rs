use serde::{Deserialize, Serialize};

use super::{
    Actor, ChainId, ReviewMode, StatusReason, SubtaskId, TicketId, TicketKey, TicketStatus,
    Timestamp,
};

/// One unit of work for one ship.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ticket {
    pub id: TicketId,
    pub chain_id: ChainId,
    pub key: TicketKey,
    pub title: String,
    /// The task given to the agent; the only source of what to do.
    pub prompt: String,
    /// Acceptance criteria, one per entry.
    pub acceptance: Vec<String>,
    pub status: TicketStatus,
    /// Present exactly when the status carries a reason (Waiting, Blocked, Cancelled).
    pub status_reason: Option<StatusReason>,
    pub status_since: Timestamp,
    /// Paused tickets stay in Backlog and are skipped by autostart.
    pub paused: bool,
    /// Agent profile name; `None` means the project default.
    pub profile: Option<String>,
    /// `None` means the chain's, then the project's review mode.
    pub review_mode: Option<ReviewMode>,
    /// Ship branch `ship/<chain>/<slug>`; empty until phase 1 creates ships.
    pub branch: Option<String>,
    /// Copy depot secrets onto the ship at start.
    pub needs_secrets: bool,
    pub tokens_used: u64,
}

/// What a person or a chain file gives for a new ticket; the rest is set by
/// [`Ticket::new_backlog`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TicketDraft {
    pub title: String,
    pub prompt: String,
    pub acceptance: Vec<String>,
    pub paused: bool,
    pub profile: Option<String>,
    pub review_mode: Option<ReviewMode>,
    pub needs_secrets: bool,
}

impl Ticket {
    /// A new ticket in Backlog, as every ticket starts; the graph decides when it is Ready.
    pub fn new_backlog(
        id: TicketId,
        chain_id: ChainId,
        key: TicketKey,
        draft: TicketDraft,
        now: Timestamp,
    ) -> Self {
        Self {
            id,
            chain_id,
            key,
            title: draft.title,
            prompt: draft.prompt,
            acceptance: draft.acceptance,
            status: TicketStatus::Backlog,
            status_reason: None,
            status_since: now,
            paused: draft.paused,
            profile: draft.profile,
            review_mode: draft.review_mode,
            branch: None,
            needs_secrets: draft.needs_secrets,
            tokens_used: 0,
        }
    }
}

/// Fields a person may change on an existing ticket; `None` keeps the current value. Status is
/// not among them: it moves only through the state machine.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema), schema(as = PatchTicket))]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TicketPatch {
    pub title: Option<String>,
    pub prompt: Option<String>,
    pub acceptance: Option<Vec<String>>,
    pub profile: Option<String>,
    pub review_mode: Option<ReviewMode>,
    pub needs_secrets: Option<bool>,
}

impl TicketPatch {
    /// Writes the given fields into `t`.
    pub fn apply(self, t: &mut Ticket) {
        if let Some(v) = self.title {
            t.title = v;
        }
        if let Some(v) = self.prompt {
            t.prompt = v;
        }
        if let Some(v) = self.acceptance {
            t.acceptance = v;
        }
        if let Some(v) = self.profile {
            t.profile = Some(v);
        }
        if let Some(v) = self.review_mode {
            t.review_mode = Some(v);
        }
        if let Some(v) = self.needs_secrets {
            t.needs_secrets = v;
        }
    }
}

/// A checklist item inside a ticket. Subtasks have no runners of their own.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Subtask {
    pub id: SubtaskId,
    pub ticket_id: TicketId,
    /// 1-based position in the checklist.
    pub position: u32,
    pub text: String,
    pub done: bool,
    /// Who ticked it, when done.
    pub done_by: Option<Actor>,
}
