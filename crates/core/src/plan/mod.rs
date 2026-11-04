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
mod parse;
mod raw;

pub use export::export;
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
    pub paused: bool,
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
