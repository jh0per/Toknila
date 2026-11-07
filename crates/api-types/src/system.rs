//! The machine: the folder check of a new project and the `tok` command next to the daemon.

use serde::{Deserialize, Serialize};

/// `POST /v1/repos/check`: the folder a person is about to make a project of.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoCheckRequest {
    pub repo_path: String,
    /// The base branch to look for; default `develop`.
    pub work_branch: Option<String>,
}

/// `GET /v1/system/cli` and `POST /v1/system/cli/install`: the version of Toknila, the `tok`
/// command that ships next to the daemon and whether it is linked into `~/.local/bin`.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CliLink {
    /// Version of the running daemon.
    pub version: String,
    /// The `tok` binary next to the daemon, when there is one.
    pub bundled: Option<String>,
    /// Where the link goes (`~/.local/bin/tok`), when the home folder is known.
    pub link: Option<String>,
    /// The link exists and points at the bundled `tok`.
    pub installed: bool,
    /// The link's folder is on the daemon's `PATH`.
    pub on_path: bool,
}
