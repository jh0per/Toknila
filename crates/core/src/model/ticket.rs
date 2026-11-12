use serde::{Deserialize, Serialize};

use crate::design::DesignRef;

use super::{
    Actor, ChainId, NodeRef, PullRequest, Review, ReviewMode, StatusReason, SubtaskId, TicketId,
    TicketKey, TicketStatus, Timestamp,
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
    /// A person asked to start this Ready card; it runs as soon as a slot is free (see
    /// `schedule`). Only a Ready card has it. Its place in the queue is computed, never stored.
    #[serde(default)]
    pub queued: bool,
    /// Set on a Waiting card whose agent's process was ended after a grace: it holds no slot and
    /// comes back when the answer arrives. Only a Waiting card has it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_paused: Option<super::PausedAgent>,
    /// Agent profile name; `None` means the project default.
    pub profile: Option<String>,
    /// `None` means the chain's, then the project's review mode.
    pub review_mode: Option<ReviewMode>,
    /// Reviewer agent profile; `None` means the chain's, then the project's, then
    /// `<worker profile>-reviewer`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reviewer: Option<String>,
    /// Ship branch `ship/<chain>/<slug>`; `null` until the card's ship is made.
    pub branch: Option<String>,
    /// Copy depot secrets onto the ship at start.
    pub needs_secrets: bool,
    /// The person wants to see and approve a design (a thread version) before any code: the agent
    /// is told to open a design thread first, and `finished` is refused until one is approved.
    #[serde(default, skip_serializing_if = "is_false")]
    pub design_first: bool,
    pub tokens_used: u64,
    /// Set on a wildcard card (work nobody planned): where it came from and whether a person
    /// approved it. It stays after the decision. Absent on planned tickets.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wildcard: Option<Wildcard>,
    /// Set when the card was taken to Merged while cards before it were still open (a spec
    /// folder whose work was done in another order). It stays after the card moves on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub done_out_of_order: Option<OutOfOrder>,
    /// The pull request of a card that merges through GitHub: opened when the card was accepted,
    /// its checks refreshed while it is watched. It stays after the card merged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pr: Option<PullRequest>,
    /// Whose verdict the card waits for in Review, and the rounds held so far.
    #[serde(default, skip_serializing_if = "Review::is_empty")]
    pub review: Review,
    /// Where a person put the card on its chain's canvas (canvas pixels); `None` leaves the card
    /// to the automatic layout. Only the window reads it: it is not in chain files.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pos: Option<Pos>,
    /// The size a person gave the card on the canvas (canvas pixels); `None` lets the card fit its
    /// text. Only the window reads it: it is not in chain files.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<Size>,
    /// The designs of the project this card is about, with the part of each that matters; the
    /// first one is the card's thumbnail on the diagram.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub designs: Vec<DesignRef>,
}

/// The width and height of a card on a chain's canvas, in whole canvas pixels.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Size {
    pub w: i32,
    pub h: i32,
}

/// A place on a chain's canvas, in whole canvas pixels.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pos {
    pub x: i32,
    pub y: i32,
}

/// The mark of a card that was done out of order: the earlier cards that were still open.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutOfOrder {
    pub behind: Vec<TicketKey>,
    /// Earlier cards whose leftovers a person chose to leave as they are.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dismissed: Vec<TicketKey>,
}

impl OutOfOrder {
    /// The mark for a card done behind `behind`, nothing dismissed yet.
    pub fn new(behind: Vec<TicketKey>) -> Self {
        Self {
            behind,
            dismissed: Vec::new(),
        }
    }

    /// Whether `card`'s leftovers still wait for a decision: it was behind, and was not dismissed.
    pub fn awaits(&self, card: &TicketKey) -> bool {
        self.behind.contains(card) && !self.dismissed.contains(card)
    }

    /// Records that the person left `card`'s leftovers as they are.
    pub fn dismiss(&mut self, card: &TicketKey) {
        if self.behind.contains(card) && !self.dismissed.contains(card) {
            self.dismissed.push(card.clone());
        }
    }
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
    pub design_first: bool,
    pub designs: Vec<DesignRef>,
}

fn is_false(v: &bool) -> bool {
    !*v
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
            queued: false,
            agent_paused: None,
            profile: draft.profile,
            review_mode: draft.review_mode,
            reviewer: draft.reviewer,
            branch: None,
            needs_secrets: draft.needs_secrets,
            design_first: draft.design_first,
            tokens_used: 0,
            wildcard: None,
            done_out_of_order: None,
            pr: None,
            review: Review::default(),
            pos: None,
            size: None,
            designs: draft.designs,
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
    pub design_first: Option<bool>,
    /// The card's designs, replacing the whole list (an empty list clears it); left out to keep
    /// them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub designs: Option<Vec<DesignRef>>,
    /// Where the card sits on the canvas: a place, `null` to hand it back to the automatic
    /// layout, left out to keep it.
    #[serde(
        default,
        deserialize_with = "some_or_null",
        skip_serializing_if = "Option::is_none"
    )]
    #[cfg_attr(feature = "openapi", schema(value_type = Option<Pos>, nullable))]
    pub pos: Option<Option<Pos>>,
    /// The card's own size: a size, `null` to let it fit its text again, left out to keep it.
    #[serde(
        default,
        deserialize_with = "some_or_null",
        skip_serializing_if = "Option::is_none"
    )]
    #[cfg_attr(feature = "openapi", schema(value_type = Option<Size>, nullable))]
    pub size: Option<Option<Size>>,
}

/// Tells a field that is `null` from one that is left out: `Some(None)` against `None`.
fn some_or_null<'de, T: Deserialize<'de>, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Option<Option<T>>, D::Error> {
    Option::<T>::deserialize(d).map(Some)
}

impl TicketPatch {
    /// Whether the patch only moves or resizes the card on the canvas (allowed on a finished card
    /// too).
    pub fn only_moves(&self) -> bool {
        (self.pos.is_some() || self.size.is_some())
            && Self {
                pos: None,
                size: None,
                ..self.clone()
            } == Self::default()
    }

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
        if let Some(v) = self.design_first {
            t.design_first = v;
        }
        if let Some(v) = self.designs {
            t.designs = v;
        }
        if let Some(v) = self.pos {
            t.pos = v;
        }
        if let Some(v) = self.size {
            t.size = v;
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
