//! Toknila application layer: use cases that run inside one unit of work.
//!
//! Every function here takes a port ([`ports::Reader`] or [`ports::UnitOfWork`]) instead of a
//! database or HTTP type, so the same rules serve the daemon's HTTP API, its startup recovery and
//! tests. Domain rules stay in `toknila-core`; this crate orders them into commands.

pub mod agent;
pub mod chain;
pub mod chains;
pub mod checks;
pub mod discussion;
pub(crate) mod epic;
pub mod error;
pub mod external;
pub mod hand_in;
pub mod inbox;
pub mod instructions;
pub mod leftovers;
pub mod merges;
pub mod moves;
pub mod notes;
pub mod nudge;
pub mod pins;
pub mod planner;
pub mod ports;
pub mod projects;
pub mod questions;
pub mod recovery;
pub mod releases;
pub mod removal;
pub mod resolve;
pub mod reviews;
pub mod runner;
pub mod runs;
pub mod schedule;
pub mod ships;
pub mod strays;
pub mod sync;
pub mod threads;
pub mod tickets;
pub mod wildcards;

pub use chain::{ChainTx, with_chain};
pub use checks::{
    CheckError, CheckGate, CheckProgress, CheckResult, CheckSpec, CheckStage, Checker, NoProgress,
    OpenGate,
};
pub use error::{AppError, AppResult, ErrorKind, GitError};
pub use merges::{MergeError, MergeOutcome, Merger};
pub use ports::{Reader, UnitOfWork};
pub use runner::{Runner, RunnerError};
pub use ships::{BranchFate, ShipError, Shipyard};
pub use sync::{SyncOutcome, SyncSpec};

#[cfg(test)]
mod tests;
