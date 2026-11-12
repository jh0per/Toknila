//! What the daemon's check command said about a hand-in, kept on the card's review so the reviewer
//! agent can be given it instead of running the gate again.

use serde::{Deserialize, Serialize};

use super::Timestamp;

/// The result of the project's check command on one hand-in.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckOutcome {
    /// The command that ran; empty when the checker never said which.
    #[serde(default)]
    pub command: String,
    pub passed: bool,
    /// The command and the end of its output, as the worker reads it.
    pub output: String,
    /// When the check finished.
    pub at: Timestamp,
}
