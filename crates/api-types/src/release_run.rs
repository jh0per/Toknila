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
    /// The steps the run will take, in order, with the push when `push` is on.
    pub steps: Vec<ReleaseStep>,
    /// The url of the repository's remote (`origin`, else its only remote), when the daemon can
    /// read one; `None` means the tag can only stay local.
    pub remote: Option<String>,
    /// The dialog's default for pushing the base branch and the tag: on in github mode and when
    /// the base branch has an upstream, and only when there is a remote.
    pub push: bool,
    /// The branch the release is made on.
    pub base: String,
    /// The last run of the chain's own release, read from the journal; `None` before the first.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<ReleaseRun>,
}

/// Body of `POST /v1/chains/{chain}/release`: the person's click on Release.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct StartRelease {
    /// The version, such as `1.4.0`; it becomes the tag `v1.4.0`.
    pub version: String,
    /// Run an interrupted release again from the step it stopped at; what passed is not redone.
    #[serde(default)]
    pub resume: bool,
    /// Push the base branch and the tag to the remote; left out, the offer's default. Asking for
    /// it with no remote is refused. A resumed run keeps the steps it began with.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub push: Option<bool>,
}

/// Body of `POST /v1/chains/{chain}/release/mark`: a person says an epic shipped in an old tag.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct MarkRelease {
    /// The tag that exists in the repository, such as `v0.3.3`.
    pub tag: String,
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
    /// The daemon stopped while the step ran.
    Interrupted,
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
    /// What the step waits for while it cannot start, read when the run is shown: `waits for
    /// its turn (#2 in the check line)`, `waits for DEMO-3 to merge (#2 in line)`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wait: Option<String>,
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
    /// The daemon stopped while the run was going; it can be run again from `resume_from`.
    Interrupted,
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
    /// The step an interrupted run was in; only when it is interrupted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resume_from: Option<ReleaseStep>,
    /// The run took its turn in its project's merge line; a running run without it waits for
    /// the merge in progress, and no card merges while it holds the turn (until its Tag and
    /// Push ended).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub turn: bool,
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
