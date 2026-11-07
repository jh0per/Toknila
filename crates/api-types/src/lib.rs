//! Request and response bodies of the Toknila daemon API: what crosses the wire between the
//! daemon and its clients (CLI, web page, desktop shell). Command inputs belong to the use cases
//! in `toknila-app` and are re-exported here; this crate adds the read models, errors and journal
//! entries.

mod planner;
mod plans;
pub mod questions;
mod removal;
pub mod runner;
mod system;
mod usage;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use toknila_core::graph::{ChainSummary, ChainView};
use toknila_core::machine::TicketAction;
use toknila_core::profile::{Catalog, ListedProfile, ProfileProblem};
use toknila_core::schedule::Plan;
use toknila_core::{
    Actor, Chain, Comment, Edge, EdgeKind, NodeRef, Phase, Project, ProjectId, Release, ReleaseId,
    Ship, Subtask, Thread, ThreadMessage, ThreadVersion, Ticket, TicketId, Timestamp,
};

pub use planner::{PlannerState, StartPlanner};
pub use plans::{ChainFile, FolderNote, ImportPath, ImportPathResult, ImportedChain};
pub use questions::{NewAnswer, PermissionAsk, PermissionDecision, PermissionReply};
pub use removal::{DeleteProject, ProjectDeletion};
pub use system::{CliLink, RepoCheckRequest};
pub use toknila_app::chains::NewChain;
pub use toknila_app::planner::{
    CardEdit, NewCard, NewPlanChain, PlanCard, PlanChain, PlanThread, PlanView, SubtaskEdit,
};
pub use toknila_app::projects::{NewProject, PatchProject};
pub use toknila_app::releases::NewRelease;
pub use toknila_app::reviews::{ReviewSettings, ReviewView};
pub use toknila_app::threads::{NewMessage, NewThread, NewVersion};
pub use toknila_app::tickets::{NewComment, NewTicket};
pub use toknila_app::wildcards::NewWildcard;
pub use toknila_core::TicketPatch as PatchTicket;
pub use toknila_core::agent::{AgentEvent, Proposal, ReleaseEvent};
pub use toknila_core::plans::{FoundPlan, PlanKind, PlanScan, WorkingCopyBehind};
pub use toknila_core::removal::FolderFate;
pub use toknila_core::usage::UsageFigures;
pub use usage::{PriceInfo, QuotaReport, UsageSettings, UsageView};

/// The profiles folder, the usable profiles and the files that were not.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileList {
    /// Where profile files go.
    pub folder: String,
    pub profiles: Vec<ListedProfile>,
    pub problems: Vec<ProfileProblem>,
}

impl ProfileList {
    /// The catalog of the profiles folder `folder`.
    pub fn new(folder: String, catalog: Catalog) -> Self {
        Self {
            folder,
            profiles: catalog.profiles,
            problems: catalog.problems,
        }
    }
}

/// A chain with its graph and computed status.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainDetail {
    pub chain: Chain,
    pub tickets: Vec<Ticket>,
    pub releases: Vec<Release>,
    pub edges: Vec<Edge>,
    pub release_tickets: Vec<ReleaseMember>,
    pub summary: ChainSummary,
    /// Nodes in dependency order.
    pub order: Vec<NodeRef>,
    /// Estimated token usage per card, by ticket key; cards that used none are absent.
    #[serde(default)]
    pub usage: BTreeMap<String, UsageFigures>,
}

/// One ticket that went into a release.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseMember {
    pub release: ReleaseId,
    pub ticket: TicketId,
}

/// A chain in a list.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainItem {
    pub chain: Chain,
    pub summary: ChainSummary,
}

/// A ticket with everything attached to it.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TicketDetail {
    pub ticket: Ticket,
    pub chain: String,
    pub subtasks: Vec<Subtask>,
    pub comments: Vec<Comment>,
    /// Milliseconds in the current status.
    pub in_status_ms: u64,
    /// Computed from the ticket's threads: design while any is open, implement otherwise.
    pub phase: Phase,
    /// The ticket's worktree and branch while it has them; `null` before it started, after it
    /// merged, or when its project has no repository.
    pub ship: Option<Ship>,
    /// How the card is reviewed (mode, reviewer, whose turn) and the rounds so far.
    pub review: ReviewView,
    /// What a person can do with the ticket now, computed by the state machine.
    pub allowed_actions: Vec<TicketAction>,
    /// Can the title and description still be changed? Decided by core from the status.
    pub can_edit: bool,
    /// Can the card still be moved in the chain? Decided by core from the status.
    pub can_move: bool,
    /// Place in line (1 is next) while the card waits for a free agent slot; `null` otherwise.
    pub queue_position: Option<u32>,
    /// Tokens the card's agents used and their estimated cost at API list prices; all zero
    /// before any run reported a transcript.
    #[serde(default)]
    pub usage: UsageFigures,
}

/// A card waiting for an agent slot.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueuedCard {
    pub ticket: TicketId,
    /// 1 is next.
    pub position: u32,
}

/// `GET /v1/agents`: one project's agent slots and the cards waiting for one.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectAgents {
    pub project: ProjectId,
    /// Agents that may work at once.
    pub limit: u32,
    /// Slots held now (cards Running or Waiting). Can exceed `limit` after the limit was lowered.
    pub used: u32,
    /// Waiting cards, first in line first.
    pub queued: Vec<QueuedCard>,
}

impl ProjectAgents {
    /// The slots of `project` as `plan` computed them.
    pub fn new(project: &Project, plan: &Plan) -> Self {
        let queued = plan
            .queue
            .iter()
            .zip(1..)
            .map(|(ticket, position)| QueuedCard {
                ticket: *ticket,
                position,
            });
        Self {
            project: project.id,
            limit: plan.limit,
            used: plan.used,
            queued: queued.collect(),
        }
    }
}

/// A thread with its messages and artifact versions.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadDetail {
    pub thread: Thread,
    /// Oldest first.
    pub messages: Vec<ThreadMessage>,
    /// By number; immutable.
    pub versions: Vec<ThreadVersion>,
    /// The thread is open and a person's request for changes has no newer version yet.
    pub changes_requested: bool,
}

/// `GET /v1/tickets/{ticket}/threads`: every thread of a ticket and the ticket's phase.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TicketThreads {
    /// The ticket's key.
    pub ticket: String,
    pub phase: Phase,
    pub threads: Vec<ThreadDetail>,
}

/// A ticket in a list.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TicketItem {
    pub ticket: Ticket,
    pub chain: String,
    pub in_status_ms: u64,
    pub needs_human: bool,
    /// Place in line (1 is next) while the card waits for a free agent slot; `null` otherwise.
    pub queue_position: Option<u32>,
}

/// Filters for `GET /v1/tickets`.
#[cfg_attr(feature = "openapi", derive(utoipa::IntoParams))]
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct TicketQuery {
    /// Chain id or name.
    pub chain: Option<String>,
    /// Only tickets waiting on a person (Proposed, Waiting, Blocked, Review).
    #[serde(default)]
    pub needs_human: bool,
}

/// `POST /v1/chains/{chain}/edges` and the query of `DELETE`.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema, utoipa::IntoParams))]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EdgeInput {
    /// Ticket key or release version.
    pub from: String,
    /// Ticket key or release version.
    pub to: String,
    #[serde(default)]
    pub kind: EdgeKind,
}

/// `POST /v1/chains/{chain}/tickets/{ticket}/move`: put the card directly before or after
/// another card of the chain.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MoveInput {
    /// Ticket key of the card the moved card goes next to.
    pub anchor: String,
    pub place: toknila_core::graph::Place,
}

/// Body of ticket actions. `text` is required by block, return and cancel.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ActionInput {
    pub text: Option<String>,
}

/// `POST /v1/tickets/{ticket}/subtasks`.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NewSubtask {
    pub text: String,
}

/// `PATCH /v1/tickets/{ticket}/subtasks/{position}`.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PatchSubtask {
    pub done: Option<bool>,
    pub text: Option<String>,
}

/// Query of `GET /v1/events`.
#[cfg_attr(feature = "openapi", derive(utoipa::IntoParams))]
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct EventQuery {
    /// Return events with seq greater than this.
    #[serde(default)]
    pub since: i64,
    /// At most this many (default 1000).
    pub limit: Option<usize>,
}

/// Body of `POST /v1/tickets/{ticket}/activity`: a sign of life from an agent's hook.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct NewActivity {
    /// What fired, for example `hook:stop`.
    pub signal: String,
    /// Claude Code's session id, from the hook's input.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// The session's transcript file, from the hook's input.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transcript_path: Option<String>,
    /// A `PreToolUse` hook: the summary of the tool call that is about to run (the command or the
    /// file, at most 300 characters), kept to say what a later permission prompt is about.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    /// A permission prompt notification: its message. The ticket's worker then waits for the
    /// person; a reviewer's prompt is listed in the inbox.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
}

impl NewActivity {
    /// A bare sign of life.
    pub fn signal(signal: impl Into<String>) -> Self {
        Self {
            signal: signal.into(),
            session_id: None,
            transcript_path: None,
            tool: None,
            prompt: None,
        }
    }
}

/// Answer of `POST /v1/tickets/{ticket}/instructions/claim`: what a person told the agent while it
/// worked, now handed over and marked delivered.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct InstructionClaim {
    /// How many instructions were handed over; 0 when nothing was pending.
    pub count: usize,
    /// The instructions as one text for the agent, in the comment template; empty when none.
    pub reason: String,
}

/// Body of `POST /v1/tickets/{ticket}/runner/size`: the size of the panel showing the terminal.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct TerminalSize {
    pub cols: u32,
    pub rows: u32,
}

impl From<(ReleaseId, TicketId)> for ReleaseMember {
    fn from((release, ticket): (ReleaseId, TicketId)) -> Self {
        Self { release, ticket }
    }
}

impl ChainDetail {
    /// The read model of a chain at `at`: its graph, its summary and its nodes in order.
    pub fn from_view(view: ChainView, at: Timestamp) -> Self {
        let summary = view.summary(at);
        let order = view.topo_order();
        Self {
            release_tickets: view
                .release_tickets
                .iter()
                .copied()
                .map(ReleaseMember::from)
                .collect(),
            chain: view.chain,
            tickets: view.tickets,
            releases: view.releases,
            edges: view.edges,
            summary,
            order,
            usage: BTreeMap::new(),
        }
    }
}

/// Error body of every failed request.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorBody {
    /// Human-readable message.
    pub error: String,
    /// Stable machine code: not_found, bad_request, conflict, ambiguous, internal, unauthorized.
    pub code: String,
    /// The input the error belongs to (`repo_path`, `work_branch`, `default_profile`), when it
    /// is about one field; a form shows the message next to that field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
}

/// One journal entry. `seq` is increasing; clients resume from it.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventRecord {
    pub seq: i64,
    pub at: Timestamp,
    pub actor: Actor,
    pub entity: String,
    pub entity_id: String,
    pub kind: String,
    #[cfg_attr(feature = "openapi", schema(value_type = Object))]
    pub payload: serde_json::Value,
}
