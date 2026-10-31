use serde::{Deserialize, Serialize};

use super::{ProjectId, ProjectKey};

/// Who reviews a finished ticket before it merges.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewMode {
    /// A person reviews the diff.
    Human,
    /// A reviewer agent reviews the diff.
    Agent,
    /// The reviewer agent first, then a person sees the diff with its remarks.
    Both,
}

/// One product: one monorepo, its chains, depot and settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    pub id: ProjectId,
    pub key: ProjectKey,
    pub name: String,
    /// Path to the repository on the host; empty until a repo is attached.
    pub repo_path: Option<String>,
    /// Branch ships start from and merge into (`develop`).
    pub work_branch: String,
    /// Branch that only receives tagged releases (`main`).
    pub release_branch: String,
    pub default_profile: String,
    pub default_review_mode: ReviewMode,
}
