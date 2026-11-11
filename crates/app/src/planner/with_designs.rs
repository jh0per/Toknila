//! A planner run of a project that has designs: the same request, plus the design files the
//! planner is told to look at first and may read.

use std::path::PathBuf;

use toknila_core::{Actor, AgentProfile, PlannerSite, Project};

use super::{person_only, request_at, runner_error};
use crate::AppResult;
use crate::runner::{RunRequest, Runner, Started};

/// [`request_at`] for a project whose design files are `designs` (absolute paths).
pub fn request_with(
    project: &Project,
    profile: &AgentProfile,
    site: Option<PlannerSite>,
    designs: Vec<PathBuf>,
) -> AppResult<RunRequest> {
    let mut req = request_at(project, profile, site)?;
    req.brief.designs = designs;
    Ok(req)
}

/// Starts the project's planner like [`super::start`], with the design files it may read. A
/// person's action.
pub fn start_with(
    runner: &dyn Runner,
    actor: Actor,
    project: &Project,
    profile: &AgentProfile,
    site: Option<PlannerSite>,
    designs: Vec<PathBuf>,
) -> AppResult<Started> {
    person_only(actor, "start the planner")?;
    runner
        .start(&request_with(project, profile, site, designs)?)
        .map_err(runner_error)
}
