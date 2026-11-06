//! What can happen to a ticket ([`TicketEvent`]) and what the caller must do after it
//! ([`Effect`]). The transition table is in [`super::apply`].

use serde::{Deserialize, Serialize};

use crate::{
    Actor, BlockedReason, NodeRef, ReviewMode, TicketKey, Verdict, WaitingReason,
    thread::OpenThread,
};

/// Something that happened to a ticket.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum TicketEvent {
    /// Graph: every dependency is satisfied.
    DepsSatisfied,
    /// Graph: a dependency is no longer satisfied (it was reopened).
    DepsUnsatisfied,
    /// A person started the card (`tok start`): it wants a runner. It runs once the schedule has
    /// a free slot, and waits in line until then (`Ticket::queued`).
    Request,
    /// The schedule gave the card a slot: its runner starts.
    Start,
    /// Agent: asked a question, wants permission, or went silent.
    AskHuman { reason: WaitingReason },
    /// A person's new comment reached the agent.
    InstructionDelivered,
    /// Agent: shows life again after a wait (an MCP event, a tool ran): back to Running.
    AgentActive,
    /// Agent: done, a diff is ready. The summary becomes a report comment.
    Finished {
        summary: String,
        /// Still-open threads of the ticket; the hand-in is refused unless this is empty.
        #[serde(skip)]
        open_threads: Vec<OpenThread>,
        /// How the card is reviewed, resolved by the caller (ticket, chain, project); it
        /// decides who the card waits for in Review.
        #[serde(skip)]
        mode: ReviewMode,
    },
    /// Agent crash, runner loss, dependency reopened, merge conflict, or `tok block`.
    Block { reason: BlockedReason },
    /// `tok unblock`.
    Unblock,
    /// `tok return` with a comment for the agent.
    Return { comment: String },
    /// `tok accept`, with remarks if the person has any.
    Accept { comment: Option<String> },
    /// Reviewer agent: its decision about the work. In agent mode it moves the card (approve
    /// merges, request-changes returns); in both mode the card then waits for a person.
    ReviewVerdict { verdict: Verdict, comment: String },
    /// The reviewer agent could not start, or ended without a verdict: a person decides.
    ReviewerFailed { detail: String },
    /// Import: the work was already done although cards before this one are still open. The
    /// card is Merged and keeps the keys of those cards as its mark.
    DoneOutOfOrder { behind: Vec<TicketKey> },
    /// Host: the ship's branch is merged into the base branch (or there was nothing to merge).
    /// Only a card whose merge is running takes it; the card is then Merged.
    MergeDone,
    /// Shipyard: the ticket's ship exists; the ticket learns its branch. No status change.
    ShipReady { branch: String },
    /// Graph: the milestone this ticket belongs to was released.
    Released,
    /// `tok reopen`.
    Reopen,
    /// `tok pause`.
    Pause,
    /// `tok resume`.
    Resume,
    /// `tok cancel <reason>`.
    Cancel { reason: String },
    /// `tok approve`: a person lets a wildcard card into the plan.
    Approve { by: Actor },
    /// `tok reject <reason>`: a person turns a wildcard card down.
    Reject { reason: String, by: Actor },
}

impl TicketEvent {
    /// Short event name for errors and logs.
    pub fn name(&self) -> &'static str {
        match self {
            Self::DepsSatisfied => "deps_satisfied",
            Self::DepsUnsatisfied => "deps_unsatisfied",
            Self::Request => "start",
            Self::Start => "start",
            Self::AskHuman { .. } => "ask_human",
            Self::InstructionDelivered => "instruction_delivered",
            Self::AgentActive => "agent_active",
            Self::Finished { .. } => "finished",
            Self::Block { .. } => "block",
            Self::Unblock => "unblock",
            Self::Return { .. } => "return",
            Self::Accept { .. } => "accept",
            Self::ReviewVerdict { .. } => "review_verdict",
            Self::ReviewerFailed { .. } => "reviewer_failed",
            Self::DoneOutOfOrder { .. } => "done_out_of_order",
            Self::MergeDone => "merge_done",
            Self::ShipReady { .. } => "ship_ready",
            Self::Released => "released",
            Self::Reopen => "reopen",
            Self::Pause => "pause",
            Self::Resume => "resume",
            Self::Cancel { .. } => "cancel",
            Self::Approve { .. } => "approve",
            Self::Reject { .. } => "reject",
        }
    }
}

/// Work the caller must do after a transition. Core decides; the daemon and graph execute.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "effect", rename_all = "snake_case")]
pub enum Effect {
    /// Make sure a runner is working on the ticket (start or restart it).
    EnsureRunner,
    /// Make sure the ticket has its ship (worktree and branch); reuse it if it exists.
    PrepareShip,
    /// Merge the ticket's ship branch into the project's base branch. The outcome comes back as
    /// `MergeDone` or as a block (`merge_conflict`, `base_checkout_dirty`, `merge_failed`).
    Merge,
    /// Take the ticket's worktree away; the branch stays.
    RemoveShip,
    /// Stop the ticket's runners, worker and reviewer, if any.
    StopRunner,
    /// Run the reviewer agent next to the worker (its own runner, on the same ship).
    StartReviewer,
    /// Stop the reviewer's runner, if any; the worker keeps running.
    StopReviewer,
    /// Store the agent's summary as a report comment.
    RecordReport { summary: String },
    /// Store a person's text as an instruction comment for the agent.
    RecordInstruction { text: String },
    /// Store the reviewer agent's remarks as an instruction comment for the worker.
    RecordRemarks { text: String },
    /// The ticket now counts as done for its dependents (Merged or Cancelled): recompute Ready.
    DependencySatisfied,
    /// The ticket left Merged: dependents in Running go Blocked, in Ready go back to Backlog.
    DependencyReopened,
    /// The ticket may have become startable (resumed): recompute its readiness.
    RecheckReadiness,
    /// A wildcard card was approved: add the dependencies proposed with it to the graph.
    AddProposedEdges {
        after: Vec<NodeRef>,
        before: Vec<NodeRef>,
    },
}
