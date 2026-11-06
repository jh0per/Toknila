use serde::{Deserialize, Serialize};

use super::{
    Actor, ChainId, NodeRef, Review, ReviewMode, StatusReason, SubtaskId, TicketId, TicketKey,
    TicketStatus, Timestamp,
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
    /// Reviewer agent profile; `None` means the chain's, then the project's, then
    /// `<worker profile>-reviewer`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reviewer: Option<String>,
    /// Ship branch `ship/<chain>/<slug>`; empty until phase 1 creates ships.
    pub branch: Option<String>,
    /// Copy depot secrets onto the ship at start.
    pub needs_secrets: bool,
    pub tokens_used: u64,
    /// Set on a wildcard card (work nobody planned): where it came from and whether a person
    /// approved it. It stays after the decision. Absent on planned tickets.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wildcard: Option<Wildcard>,
    /// Set when the card was taken to Merged while cards before it were still open (a spec
    /// folder whose work was done in another order). It stays after the card moves on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub done_out_of_order: Option<OutOfOrder>,
    /// Whose verdict the card waits for in Review, and the rounds held so far.
    #[serde(default, skip_serializing_if = "Review::is_empty")]
    pub review: Review,
}

/// The mark of a card that was done out of order: the earlier cards that were still open.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutOfOrder {
    pub behind: Vec<TicketKey>,
}

/// The mark of a wildcard card: work found during the chain that waits for a person's approval
/// before it joins the plan.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Wildcard {
    /// The ticket during which the work was found.
    pub from: Option<TicketKey>,
    /// Who proposed it.
    pub proposed_by: Actor,
    /// Nodes it is to wait for; they become edges when it is approved.
    pub after: Vec<NodeRef>,
    /// Nodes that are to wait for it; they become edges when it is approved.
    pub before: Vec<NodeRef>,
    /// A person approved it, so it is part of the plan.
    pub approved: bool,
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
    pub reviewer: Option<String>,
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
            reviewer: draft.reviewer,
            branch: None,
            needs_secrets: draft.needs_secrets,
            tokens_used: 0,
            wildcard: None,
            done_out_of_order: None,
            review: Review::default(),
        }
    }

    /// A new wildcard card in Proposed: it waits for a person before it joins the plan.
    pub fn new_proposed(
        id: TicketId,
        chain_id: ChainId,
        key: TicketKey,
        draft: TicketDraft,
        wildcard: Wildcard,
        now: Timestamp,
    ) -> Self {
        Self {
            status: TicketStatus::Proposed,
            status_reason: Some(StatusReason::Proposed),
            wildcard: Some(Wildcard {
                approved: false,
                ..wildcard
            }),
            ..Self::new_backlog(id, chain_id, key, draft, now)
        }
    }

    /// Part of the plan: a planned ticket, or a wildcard card a person approved. Only these
    /// count in chain progress and go into chain files.
    pub fn in_plan(&self) -> bool {
        self.wildcard.as_ref().is_none_or(|w| w.approved)
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
    pub reviewer: Option<String>,
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
        if let Some(v) = self.reviewer {
            t.reviewer = Some(v).filter(|v| !v.trim().is_empty());
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
