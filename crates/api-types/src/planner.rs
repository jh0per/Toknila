//! A project's planner as the window and the CLI see it: whether its process is alive.

use serde::{Deserialize, Serialize};
use toknila_core::RunId;

/// `GET /v1/projects/{project}/planner`: the project's planner.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannerState {
    /// Its process is alive.
    pub running: bool,
    /// The live run; `null` when it is not running.
    pub run: Option<RunId>,
    /// The profile it runs with (or, when it is not running, would start with).
    pub profile: String,
}

/// `POST /v1/projects/{project}/planner/start`.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartPlanner {
    /// A planner profile from `GET /v1/profiles`; unset: `claude-code-planner`.
    pub profile: Option<String>,
}
