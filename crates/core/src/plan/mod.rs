//! Chain files: TOML import and export of a chain (`tok chain import|export`).
//!
//! The database is the source of truth; a file is only a way in and out. Inside a file, nodes
//! have local ids (`A`, `R1`, or the ticket keys on export) that `after` lists refer to.
//!
//! ```toml
//! [chain]
//! name = "demo"
//! goal = "Phase-0 gate"
//!
//! [[ticket]]
//! id = "A"
//! title = "A"
//! subtasks = ["write the test"]
//! done_subtasks = [1]   # optional: positions already ticked
//! done = true           # optional: import as Merged
//!
//! [[ticket]]
//! id = "B"
//! after = ["A"]
//!
//! [[release]]
//! id = "R1"            # version defaults to the id
//! start = "manual"     # or "auto"
//! channel = "tag_only"
//! after = ["B"]
//! ```

use serde::Serialize;

use crate::{EdgeKind, ReleaseChannel, ReviewMode, StartMode};

mod export;
mod instantiate;
mod order;
mod parse;
mod raw;

pub use export::export;
pub use instantiate::Checklist;
pub use order::out_of_order;
pub use parse::parse;

/// A problem in a chain file. `line` is 1-based.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("line {line}: {message}")]
pub struct PlanError {
    pub line: usize,
    pub message: String,
}

/// Chain settings from a file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlanChain {
    pub name: String,
    pub goal: String,
    pub autostart: bool,
    pub review_mode: Option<ReviewMode>,
}

/// One ticket from a file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlanTicket {
    pub local_id: String,
    pub title: String,
    pub prompt: String,
    pub acceptance: Vec<String>,
    pub subtasks: Vec<String>,
    pub profile: Option<String>,
    pub review_mode: Option<ReviewMode>,
    pub needs_secrets: bool,
    /// Show a design for approval before any code (`design_first = true`).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub design_first: bool,
    pub paused: bool,
    /// Import as Merged: the ticket goes through the state machine once its dependencies are.
    pub done: bool,
    /// 1-based positions of subtasks that start ticked.
    pub done_subtasks: Vec<u32>,
}

/// One milestone from a file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlanRelease {
    pub local_id: String,
    pub version: String,
    pub channel: ReleaseChannel,
    pub start_mode: StartMode,
}

/// A node of a plan: index into `tickets` or `releases`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum PlanNode {
    Ticket(usize),
    Release(usize),
}

/// A dependency in a plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct PlanEdge {
    pub from: PlanNode,
    pub to: PlanNode,
    pub kind: EdgeKind,
}

/// A validated chain file: unique ids, known references, no cycles.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Plan {
    pub chain: PlanChain,
    pub tickets: Vec<PlanTicket>,
    pub releases: Vec<PlanRelease>,
    pub edges: Vec<PlanEdge>,
}

#[cfg(test)]
mod tests;
