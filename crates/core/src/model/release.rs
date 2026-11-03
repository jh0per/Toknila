use serde::{Deserialize, Serialize};

use super::{ChainId, ReleaseId, Timestamp};

/// Where a release milestone ships.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseChannel {
    /// Only merge into the release branch and tag.
    TagOnly,
    TestFlight,
    PlayInternal,
    PlayClosed,
    PlayProduction,
}

/// Whether the milestone cuts itself or waits for a person.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StartMode {
    /// Cut as soon as everything before it is Merged.
    Auto,
    /// Wait for `tok release start`.
    Manual,
}

/// Milestone lifecycle (gitflow): planned, then `release/<version>` is cut, then shipped.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseState {
    Planned,
    Cut,
    Released,
}

/// A release milestone between tickets on the chain graph.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Release {
    pub id: ReleaseId,
    pub chain_id: ChainId,
    pub version: String,
    pub channel: ReleaseChannel,
    pub start_mode: StartMode,
    pub state: ReleaseState,
    /// `release/<version>`; empty in phase 0, where there are no branches.
    pub branch: Option<String>,
    pub cut_at: Option<Timestamp>,
    pub released_at: Option<Timestamp>,
}
