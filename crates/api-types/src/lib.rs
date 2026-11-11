//! Request and response bodies of the Toknila daemon API: what crosses the wire between the
//! daemon and its clients (CLI, web page, desktop shell). Command inputs belong to the use cases
//! in `toknila-app` and are re-exported here; this crate adds the read models, errors and journal
//! entries.

mod agents;
mod chains;
mod designs;
mod events;
mod planner;
mod plans;
mod profiles;
pub mod questions;
mod release_run;
mod removal;
pub mod runner;
mod strays;
mod system;
mod threads;
mod tickets;
mod usage;

pub use agents::{ProjectAgents, QueuedCard};
pub use chains::{
    ChainDetail, ChainItem, DesignMoveInput, EdgeInput, EdgeMoveInput, MoveInput, ReleaseMember,
};
pub use designs::{
    Design, DesignList, FigmaToken, NewDesignPin, NewFigmaDesign, PlannerDesign, PlannerPlan,
    RenameDesign,
};
pub use events::{ErrorBody, EventQuery, EventRecord, InstructionClaim, NewActivity};
pub use planner::{PlannerState, StartPlanner};
pub use plans::{ChainFile, FolderNote, ImportPath, ImportPathResult, ImportedChain};
pub use profiles::ProfileList;
pub use questions::{NewAnswer, PermissionAsk, PermissionDecision, PermissionReply};
pub use release_run::{
    ReleaseOffer, ReleaseRun, RunState, RunStep, StartRelease, StepState, VersionSuggestion,
};
pub use removal::{ChainDeletion, DeleteProject, ProjectDeletion};
pub use runner::TerminalSize;
pub use strays::{Strays, StraysCleaned};
pub use system::{CliLink, Places, RepoCheckRequest};
pub use threads::{ThreadDetail, TicketThreads};
pub use tickets::{ActionInput, NewSubtask, PatchSubtask, TicketDetail, TicketItem, TicketQuery};
pub use toknila_app::chains::{ChainSettings, NewChain};
pub use toknila_app::pins::PinAnswer;
pub use toknila_app::planner::{
    CardEdit, NewCard, NewPlanChain, NewSuggestion, PlanCard, PlanChain, PlanThread, PlanView,
    SubtaskEdit,
};
pub use toknila_app::projects::{NewProject, PatchProject};
pub use toknila_app::releases::NewRelease;
pub use toknila_app::reviews::ReviewView;
pub use toknila_app::threads::{NewMessage, NewThread, NewVersion};
pub use toknila_app::tickets::{NewComment, NewTicket};
pub use toknila_app::wildcards::NewWildcard;
pub use toknila_core::TicketPatch as PatchTicket;
pub use toknila_core::agent::{AgentEvent, Proposal, ReleaseEvent};
pub use toknila_core::design::pin::DesignPin;
pub use toknila_core::plans::{FoundPlan, PlanKind, PlanScan, WorkingCopyBehind};
pub use toknila_core::removal::FolderFate;
pub use toknila_core::usage::UsageFigures;
pub use usage::{PriceInfo, QuotaReport, UsageSettings, UsageView};
