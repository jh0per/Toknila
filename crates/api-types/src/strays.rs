//! Ships and branches no live card owns (Settings, "Leftover ships").

use serde::{Deserialize, Serialize};
use toknila_core::strays::Stray;

/// Answer of `GET /v1/projects/{project}/strays`: the worktrees and `ship/*` branches in the
/// project's repository that no live card owns, left by cancelled or deleted cards.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct Strays {
    /// Worktrees first, then branches.
    pub items: Vec<Stray>,
}

/// Answer of `POST /v1/projects/{project}/strays/clean`: what was removed, and the steps that
/// failed (those leftovers stay listed).
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct StraysCleaned {
    pub removed: Vec<Stray>,
    pub warnings: Vec<String>,
}
