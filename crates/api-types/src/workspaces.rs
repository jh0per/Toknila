//! A project's workspaces: the ships of its stories and the assistant's reading copy (Settings,
//! `tok ship ls`).

use serde::{Deserialize, Serialize};
use toknila_core::TicketStatus;

/// What a workspace folder is.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceKind {
    /// A story's ship.
    Story,
    /// The assistant's reading copy of the code (`_planner`); never a leftover.
    Planner,
}

/// One workspace of a project.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct Workspace {
    pub kind: WorkspaceKind,
    /// The story's key; none for the assistant's copy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub story: Option<String>,
    /// The story's status; none for the assistant's copy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<TicketStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    pub path: String,
    /// The story is merged, released or cancelled: Clean up retires this workspace.
    pub finished: bool,
}

/// Answer of `GET /v1/projects/{project}/ships`.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct Workspaces {
    /// The assistant's copy (when it exists) first, then the stories' ships.
    pub items: Vec<Workspace>,
    /// The stories' workspaces; the assistant's copy is not counted.
    pub count: u32,
    /// Of `count`, those that belong to finished stories.
    pub finished: u32,
}

/// Answer of `POST /v1/projects/{project}/ships/retire-finished`.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct WorkspacesRetired {
    /// Keys of the stories whose workspace is being retired.
    pub retired: Vec<String>,
}
