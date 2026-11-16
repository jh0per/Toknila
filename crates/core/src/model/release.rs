use serde::{Deserialize, Serialize};

use super::{ChainId, ReleaseId, Timestamp};

/// Where a release milestone ships.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseChannel {
    /// Only merge into the release branch and tag. The default.
    #[default]
    TagOnly,
    TestFlight,
    PlayInternal,
    PlayClosed,
    PlayProduction,
}

/// Whether the milestone cuts itself or waits for a person.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StartMode {
    /// Cut as soon as everything before it is Merged.
    Auto,
    /// Wait for `tok release start`. The default.
    #[default]
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
    /// The release's branch (`release/<version>`); `null` while no branch was made.
    pub branch: Option<String>,
    pub cut_at: Option<Timestamp>,
    pub released_at: Option<Timestamp>,
    /// The git tag of the release (`v1.4.0`), set when a release run made it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    /// Set when the epic shipped inside another epic's tag: the tag and the epic that cut it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub released_with: Option<ReleasedWith>,
}

/// The tag an epic's work shipped in, and the epic whose release run (or a person) made it count.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleasedWith {
    /// The git tag (`v0.3.3`).
    pub tag: String,
    /// The name of the epic whose release run made the tag; none for a tag a person made.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub by: Option<String>,
}

impl Release {
    /// A new milestone in Planned, before any branch is cut.
    pub fn planned(
        id: ReleaseId,
        chain_id: ChainId,
        version: String,
        channel: ReleaseChannel,
        start_mode: StartMode,
    ) -> Self {
        Self {
            id,
            chain_id,
            version,
            channel,
            start_mode,
            state: ReleaseState::Planned,
            branch: None,
            cut_at: None,
            released_at: None,
            tag: None,
            released_with: None,
        }
    }
}
