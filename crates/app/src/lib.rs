//! Toknila application layer: use cases that run inside one unit of work.
//!
//! Every function here takes a port ([`ports::Reader`] or [`ports::UnitOfWork`]) instead of a
//! database or HTTP type, so the same rules serve the daemon's HTTP API, its startup recovery and
//! tests. Domain rules stay in `toknila-core`; this crate orders them into commands.

pub mod agent;
pub mod chain;
pub mod error;
pub mod notes;
pub mod ports;
pub mod recovery;
pub mod resolve;

pub use chain::{ChainTx, with_chain};
pub use error::{AppError, AppResult, ErrorKind};
pub use ports::{Reader, UnitOfWork};

#[cfg(test)]
mod tests;
