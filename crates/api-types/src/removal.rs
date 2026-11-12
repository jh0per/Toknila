//! Deleting a project.

use serde::{Deserialize, Serialize};
use toknila_core::removal::FolderFate;
use toknila_core::{Chain, Project};

/// `POST /v1/projects/{project}/delete`.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct DeleteProject {
    /// Also move the repository folder to the system Trash. Without it the folder stays.
    #[serde(default)]
    pub trash_folder: bool,
}

/// Answer of `DELETE /v1/chains/{chain}`: the chain and its cards are gone; what became of the
/// files around them. The repository folder and the `ship/*` branches stay.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct ChainDeletion {
    /// The chain as it was.
    pub chain: Chain,
    /// The words a person read for it: its title, or its name made readable.
    #[serde(default)]
    pub display_title: String,
    /// Key of the project it belonged to.
    pub project: String,
    /// How many cards went with it.
    pub tickets: usize,
    /// The ships' worktrees removed from the repository.
    pub worktrees_removed: Vec<String>,
    /// Cleanup that failed after the records were deleted. The deletion stands either way.
    pub warnings: Vec<String>,
}

/// Answer of `POST /v1/projects/{project}/delete`: the records are gone; what became of the
/// files around them.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct ProjectDeletion {
    /// The project as it was.
    pub project: Project,
    /// Names of the deleted chains.
    pub chains: Vec<String>,
    /// How many cards went with them.
    pub tickets: usize,
    /// The ships' worktrees removed from the repository; their `ship/*` branches stay.
    pub worktrees_removed: Vec<String>,
    /// What became of the repository folder: `kept`, `trashed`, or `none` (no folder).
    pub folder: FolderFate,
    /// Why the folder was not moved to the Trash when that was asked; `null` otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder_error: Option<String>,
    /// Cleanup that failed after the records were deleted (a worktree or run logs left on disk).
    /// The deletion stands either way.
    pub warnings: Vec<String>,
}
