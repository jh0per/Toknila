//! What can happen to a ticket ([`TicketEvent`]) and what the caller must do after it
//! ([`Effect`]). The transition table is in [`super::apply`].

use serde::{Deserialize, Serialize};

use crate::{
    Actor, BlockedReason, ItemVerdict, NodeRef, PullRequest, ReviewMode, StalePolicy, TicketKey,
    Verdict, WaitingReason, thread::OpenThread,
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
    /// A person answered the agent's question or permission prompt and the answer reached it:
    /// back to Running at once, without waiting for the agent's next sign of life.
    Answered,
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
        /// The daemon runs the project's check command on the ship before anyone reviews (the
        /// project names one and the card has a ship), resolved by the caller.
        #[serde(skip)]
        check: bool,
        /// The project's round limit of the loop, resolved by the caller.
        #[serde(skip)]
        max_rounds: u32,
    },
    /// Host: the check command ran on the ship. `output` is what a worker reads (the command
    /// and the end of its output). A pass goes on to whoever `mode` says; a failure sends the
    /// card back to the worker, a round, until `max_rounds` rounds were used.
    CheckDone {
        passed: bool,
        output: String,
        #[serde(skip)]
        mode: ReviewMode,
        #[serde(skip)]
        max_rounds: u32,
    },
    /// Host: the check could not run at all (no shell, no ship, the daemon restarted). A person
    /// decides.
    CheckLost { detail: String },
    /// Host: the project has no check command; the ship was only brought up to its base and that
    /// went through. The round says so instead of "the check passed".
    CheckSkipped {
        #[serde(skip)]
        mode: ReviewMode,
    },
    /// Host: the check's turn came and the ship is being brought up to the base branch.
    CheckSyncing,
    /// Host: the check command `command` started running on the ship.
    CheckRunning { command: String },
    /// Host: the check command ran but its program was not found (`toolpath::command_not_found`).
    /// That is the machine's fault, not the worker's: no round, no return; a person decides.
    CheckCannotRun { command: String },
    /// Agent crash, runner loss, dependency reopened, merge conflict, or `tok block`.
    Block { reason: BlockedReason },
    /// `tok unblock`.
    Unblock,
    /// A card blocked because its base branch is behind: move the base to its upstream, then
    /// start. Only when the block says it can.
    FastForwardStart,
    /// A card blocked because its base branch is behind: start from the base as it is.
    StartAnyway,
    /// `tok return` with a comment for the agent.
    Return { comment: String },
    /// `tok accept`, with remarks if the person has any.
    Accept { comment: Option<String> },
    /// Reviewer agent: its decision about the work. In agent mode it moves the card (approve
    /// merges, request-changes returns); in both mode the card then waits for a person.
    ReviewVerdict {
        verdict: Verdict,
        comment: String,
        /// The judgement of each acceptance item; required when the ticket has any.
        #[serde(default)]
        items: Vec<ItemVerdict>,
        /// The project's round limit, resolved by the caller.
        #[serde(skip)]
        max_rounds: u32,
    },
    /// The reviewer agent could not start, or ended without a verdict: a person decides.
    ReviewerFailed { detail: String },
    /// Host: the reviewer ended a turn (its Stop hook, or silence) while the card still waits for
    /// its verdict. `screen` is the end of its screen, ANSI removed.
    ReviewerStopped { screen: Vec<String> },
    /// `tok run-reviewer`: a person asks for the reviewer again after it gave no verdict.
    RunReviewer,
    /// Import: the work was already done although cards before this one are still open. The
    /// card is Merged and keeps the keys of those cards as its mark.
    DoneOutOfOrder { behind: Vec<TicketKey> },
    /// Host: the ship's branch is merged into the base branch (or there was nothing to merge).
    /// Only a card whose merge is running takes it; the card is then Merged.
    MergeDone,
    /// Host: the pull request of the card's merge was opened or its checks changed. Only a
    /// card whose merge is running takes it; no status change.
    /// `ask_after_secs` is the project's timeout: a pull request with no check for that long
    /// makes the card ask a person (a `no_checks` note).
    PrUpdated {
        pr: PullRequest,
        ask_after_secs: u64,
    },
    /// `tok merge-now`: the pull request reported no checks; merge it anyway.
    MergeUnchecked,
    /// `tok keep-waiting`: the pull request reported no checks; wait for the timeout again.
    KeepWaiting,
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
    /// `tok design`: a person wants a design to approve before any code. A card that has not
    /// started gets `design_first`; one that runs also gets an instruction to open a design thread.
    AskDesign,
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
            Self::Answered => "answered",
            Self::Finished { .. } => "finished",
            Self::CheckDone { .. } => "check_done",
            Self::CheckLost { .. } => "check_lost",
            Self::CheckSkipped { .. } => "check_skipped",
            Self::CheckSyncing => "check_syncing",
            Self::CheckRunning { .. } => "check_running",
            Self::CheckCannotRun { .. } => "check_cannot_run",
            Self::Block { .. } => "block",
            Self::Unblock => "unblock",
            Self::FastForwardStart => "fast_forward_start",
            Self::StartAnyway => "start_anyway",
            Self::Return { .. } => "return",
            Self::Accept { .. } => "accept",
            Self::ReviewVerdict { .. } => "review_verdict",
            Self::ReviewerFailed { .. } => "reviewer_failed",
            Self::ReviewerStopped { .. } => "reviewer_stopped",
            Self::RunReviewer => "run_reviewer",
            Self::DoneOutOfOrder { .. } => "done_out_of_order",
            Self::MergeDone => "merge_done",
            Self::PrUpdated { .. } => "pr_updated",
            Self::MergeUnchecked => "merge_now",
            Self::KeepWaiting => "keep_waiting",
            Self::ShipReady { .. } => "ship_ready",
            Self::Released => "released",
            Self::Reopen => "reopen",
            Self::Pause => "pause",
            Self::Resume => "resume",
            Self::AskDesign => "ask_design",
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
    /// [`Effect::PrepareShip`] after a person decided what to do about a base branch that is
    /// behind its upstream.
    PrepareShipWith { stale: StalePolicy },
    /// Merge the ticket's ship branch into the project's base branch. The outcome comes back as
    /// `MergeDone` or as a block (`merge_conflict`, `base_checkout_dirty`, `merge_failed`).
    Merge,
    /// Take the ticket's worktree away; the branch stays.
    RemoveShip,
    /// The ticket was cancelled: take its worktree away, delete its branch when it holds no
    /// commits of its own, keep it (and say so on the card) when it does.
    RetireShip,
    /// Stop the ticket's runners, worker and reviewer, if any.
    StopRunner,
    /// Stop the worker's runner only (the hand-in ended its run); a reviewer keeps running.
    StopWorker,
    /// Run the project's check command on the ticket's ship (bounded time, output kept). The
    /// outcome comes back as `CheckDone` or `CheckLost`.
    RunCheck,
    /// Store the failing check's output as an instruction comment for the worker.
    RecordCheckOutput { text: String },
    /// Run the reviewer agent next to the worker (its own runner, on the same ship).
    StartReviewer,
    /// Stop the reviewer's runner, if any; the worker keeps running.
    StopReviewer,
    /// Tell the live reviewer to record its verdict (it ended a turn without one).
    NudgeReviewer { message: String },
    /// Store the agent's summary as a report comment.
    RecordReport { summary: String },
    /// Store the daemon's own words (a lost check, a reviewer that gave no verdict) as a report
    /// comment written by the system, never by the agent.
    RecordNotice { summary: String },
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
