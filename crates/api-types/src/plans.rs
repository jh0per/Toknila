//! Plans into a project: a chain file as text, or a path the daemon reads (a chain file, a spec
//! folder, a folder of spec folders). The plans a repository holds are core's `PlanScan`.

use serde::{Deserialize, Serialize};

use crate::ChainDetail;

/// A chain file: body of `POST /v1/projects/{project}/import`, answer of
/// `GET /v1/chains/{chain}/export`.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ChainFile {
    pub toml: String,
}

/// `POST /v1/projects/{project}/import-path`: what the daemon is to read and import.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ImportPath {
    /// Absolute path of a TOML chain file, of a spec folder (one with `tasks.md`), or of a folder
    /// whose subfolders are spec folders.
    pub path: String,
}

/// One chain an import made.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ImportedChain {
    /// The spec folder's or the chain file's name.
    pub folder: String,
    /// Where it was read from, absolute; the chain keeps it as its `source`.
    pub path: String,
    pub cards: usize,
    /// Subtasks of all cards.
    pub tasks: usize,
    /// Cards imported as Merged because all their tasks were checked.
    pub done_cards: usize,
    /// Done cards that sit behind a card with open tasks, one line each.
    pub out_of_order: Vec<String>,
    /// The new chain, as `POST /v1/projects/{project}/import` answers it.
    pub chain: ChainDetail,
}

/// A subfolder that was not imported.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct FolderNote {
    pub folder: String,
    /// Absolute.
    pub path: String,
    /// Why: `no tasks.md`, or the error, which names the file and, for a `tasks.md`, the line.
    pub reason: String,
}

/// Answer of `POST /v1/projects/{project}/import-path`. A chain file or a spec folder makes one
/// chain or fails as a whole (400 or 409, the message names the path); a folder of spec folders
/// imports each on its own and lists what it skipped and what failed.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ImportPathResult {
    /// In folder order.
    pub chains: Vec<ImportedChain>,
    /// Subfolders without a `tasks.md`.
    pub skipped: Vec<FolderNote>,
    /// Spec folders that could not be imported; the others were.
    pub failed: Vec<FolderNote>,
}
