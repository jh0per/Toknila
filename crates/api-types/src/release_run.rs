//! A release of a whole chain: what the dialog offers, what the person confirms, and the run's
//! steps as they go.

use serde::{Deserialize, Serialize};
use toknila_core::release_run::ReleaseStep;

/// Answer of `GET /v1/chains/{chain}/release`: what the Release dialog shows.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct ReleaseOffer {
    /// Every card is merged: the end node offers Release.
    pub releasable: bool,
    /// The version to suggest: the chain's own release when it has one, else the next patch of
    /// the last tag.
    pub version: String,
    /// The last tag of the repository, if it has one.
    pub last_tag: Option<String>,
    /// The steps the run will take, in order.
    pub steps: Vec<ReleaseStep>,
    /// The branch the release is made on.
    pub base: String,
}

/// Body of `POST /v1/chains/{chain}/release`: the person's click on Release.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StartRelease {
    /// The version, such as `1.4.0`; it becomes the tag `v1.4.0`.
    pub version: String,
}

/// Where one step of a release run is.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StepState {
    Pending,
    Running,
    Done,
    Failed,
}

/// One step of a release run with what it printed.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct RunStep {
    pub step: ReleaseStep,
    pub label: String,
    pub state: StepState,
    /// What the step printed (the end of it when it is long).
    pub output: String,
}

/// Where a release run is as a whole.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RunState {
    Running,
    /// A step failed; nothing after it ran. Releasing again starts over.
    Failed,
    /// Every step passed and the release node reads released.
    Done,
}

/// Answer of `GET /v1/releases/{release}/run`: the last release run of a milestone.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct ReleaseRun {
    pub release: String,
    pub version: String,
    pub tag: String,
    pub state: RunState,
    pub steps: Vec<RunStep>,
    /// Why the run stopped, naming the step; only when it failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Answer of `GET /v1/projects/{project}/next-version`: the version a new chain suggests.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct VersionSuggestion {
    /// The next patch of the newest tag, or `0.1.0` when the repository has none.
    pub version: String,
    /// The newest tag of the repository, if it has one.
    pub last_tag: Option<String>,
}
