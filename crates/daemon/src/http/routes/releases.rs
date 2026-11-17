//! `/chains/{chain}/releases`, `/releases`.

use std::path::PathBuf;

use crate::http::body::Json;
use axum::extract::{Path, State};
use toknila_app::releases::{self, ReleaseAction};
use toknila_app::{AppError, resolve};
use toknila_core::{Actor, release_run};

use super::{ApiResult, chain_write, read, release_task};
use crate::http::dto::{
    ChainDetail, NewRelease, ReleaseOffer, ReleaseRun, StartRelease, VersionSuggestion,
};
use crate::http::error::ErrorBody;
use crate::releaser;
use crate::server::AppState;
use crate::shipyard::remote::{Remote, has_upstream, remote_of};
use crate::store::release_runs::Entry;
use toknila_api_types::{RunState, StepState};

#[utoipa::path(post, path = "/chains/{chain}/releases", tag = "releases", request_body = NewRelease,
    params(("chain" = String, Path, description = "Chain id or name")),
    responses((status = 200, body = ChainDetail), (status = 404, body = ErrorBody), (status = 409, body = ErrorBody)))]
pub async fn create_release(
    State(state): State<AppState>,
    Path(chain): Path<String>,
    Json(input): Json<NewRelease>,
) -> ApiResult<Json<ChainDetail>> {
    let detail = chain_write(&state, Actor::Human, move |tx| {
        Ok(releases::create(tx, &chain, input)?)
    })
    .await?;
    Ok(Json(detail))
}

#[utoipa::path(post, path = "/releases/{release}/actions/{action}", tag = "releases",
    params(("release" = String, Path, description = "Release id or version"), ("action" = String, Path, description = "start (cut) or complete (released)")),
    responses((status = 200, body = ChainDetail), (status = 404, body = ErrorBody), (status = 409, body = ErrorBody)))]
pub async fn release_action(
    State(state): State<AppState>,
    Path((release, action)): Path<(String, String)>,
) -> ApiResult<Json<ChainDetail>> {
    let action: ReleaseAction = action.parse()?;
    let detail = chain_write(&state, Actor::Human, move |tx| {
        let r = resolve::release(&*tx, &release)?;
        releases::act(tx, &r, action)?;
        Ok(r.chain_id)
    })
    .await?;
    Ok(Json(detail))
}

/// The steps of a release of the project, with the push when the release asks for it.
fn project_steps(
    p: &toknila_core::Project,
    push: bool,
) -> Vec<toknila_core::release_run::ReleaseStep> {
    toknila_core::release_run::steps(
        p.check_command.as_deref(),
        p.bump_command.as_deref(),
        p.artifact_command.as_deref(),
        push,
    )
}

/// What the repository offers to push to, and whether the release pushes unless told otherwise:
/// on in github mode and when the base branch has an upstream, and only with a remote.
fn push_facts(p: &toknila_core::Project) -> (Option<Remote>, bool) {
    let Some(repo) = p.repo_path.as_deref().filter(|r| !r.trim().is_empty()) else {
        return (None, false);
    };
    let git = crate::git::Git::new("git");
    let remote = remote_of(&git, repo);
    let default = remote.is_some()
        && (p.merge.mode == toknila_core::MergeMode::Github
            || has_upstream(&git, repo, &p.work_branch));
    (remote, default)
}

pub(super) fn repo_of(p: &toknila_core::Project) -> Result<String, AppError> {
    p.repo_path
        .clone()
        .filter(|r| !r.trim().is_empty())
        .ok_or_else(|| AppError::conflict("the project has no repository to release from"))
}

#[utoipa::path(get, path = "/chains/{chain}/release", tag = "releases",
    params(("chain" = String, Path, description = "Chain id or name")),
    responses((status = 200, body = ReleaseOffer), (status = 404, body = ErrorBody)))]
pub async fn release_offer(
    State(state): State<AppState>,
    Path(chain): Path<String>,
) -> ApiResult<Json<ReleaseOffer>> {
    let checks = state.store.check_line().standing();
    let (releasable, own, project, run) = read(&state, move |repo| {
        let c = resolve::chain(repo, &chain)?;
        let view = repo
            .chain_view(c.id)?
            .ok_or_else(|| AppError::not_found("chain"))?;
        let project = repo
            .project(c.project_id)?
            .ok_or_else(|| AppError::not_found("project"))?;
        let end = view.end_release();
        let own = end.map(|r| r.version.clone());
        let run = match end.map(|r| (r.id, repo.release_run(r.id))) {
            Some((id, Ok(Some(run)))) => Some(release_task::with_waits(repo, &checks, id, run)?),
            Some((_, Err(e))) => return Err(e.into()),
            _ => None,
        };
        Ok((view.releasable(), own, project, run))
    })
    .await?;
    let tags = match project.repo_path.clone().filter(|r| !r.trim().is_empty()) {
        Some(repo) => tokio::task::spawn_blocking(move || releaser::tags(&repo))
            .await
            .unwrap_or_default(),
        None => Vec::new(),
    };
    let (remote, push) = {
        let p = project.clone();
        tokio::task::spawn_blocking(move || push_facts(&p))
            .await
            .unwrap_or((None, false))
    };
    Ok(Json(ReleaseOffer {
        releasable,
        version: own.unwrap_or_else(|| release_run::suggest_version(&tags)),
        last_tag: tags.first().cloned(),
        steps: project_steps(&project, push),
        remote: remote.map(|r| r.url),
        push,
        base: project.work_branch.clone(),
        build: project
            .artifact_command
            .as_deref()
            .filter(|c| !c.trim().is_empty())
            .map(release_run::command_words),
        run,
    }))
}

#[utoipa::path(post, path = "/chains/{chain}/release", tag = "releases", request_body = StartRelease,
    params(("chain" = String, Path, description = "Chain id or name")),
    responses((status = 200, body = ChainDetail), (status = 400, body = ErrorBody), (status = 404, body = ErrorBody), (status = 409, body = ErrorBody)))]
/// The person's click on Release: cuts the chain's release milestone and starts the run in the
/// background. The answer is the chain with the milestone cut; the steps are read from
/// `GET /releases/{release}/run`, and the milestone reads released when the last one passes.
pub async fn start_release(
    State(state): State<AppState>,
    Path(chain): Path<String>,
    Json(input): Json<StartRelease>,
) -> ApiResult<Json<ChainDetail>> {
    let started = std::sync::Arc::new(std::sync::Mutex::new(None));
    let slot = started.clone();
    let detail = chain_write(&state, Actor::Human, move |tx| {
        let (r, project) = releases::begin(tx, &chain, &input.version)?;
        let repo = repo_of(&project)?;
        let version = r.version.clone();
        let last = tx.repo().release_run(r.id)?;
        if last.as_ref().is_some_and(|l| l.state == RunState::Running) {
            return Err(AppError::conflict("a release of this epic is already running").into());
        }
        let (remote, default) = push_facts(&project);
        let steps = match last.as_ref().filter(|_| input.resume) {
            // A resumed run keeps the steps it began with, the push or not.
            Some(run) => run.steps.iter().map(|s| s.step).collect(),
            None => {
                let push = input.push.unwrap_or(default);
                if push && remote.is_none() {
                    return Err(AppError::conflict(
                        "the repository has no remote to push to: the tag stays local; release without the push",
                    )
                    .into());
                }
                project_steps(&project, push)
            }
        };
        let (todo, entry) = if input.resume {
            let run = last
                .filter(|l| matches!(l.state, RunState::Interrupted | RunState::Failed))
                .ok_or_else(|| AppError::conflict("there is no release to run again"))?;
            let passed: Vec<_> = run
                .steps
                .iter()
                .filter(|s| s.state == StepState::Done)
                .map(|s| s.step)
                .collect();
            (
                release_run::resume_steps(&steps, &passed),
                Entry::RunResumed,
            )
        } else {
            let entry = Entry::RunStarted {
                version: version.clone(),
                tag: release_run::tag_name(&version),
                steps: steps.clone(),
            };
            (steps.clone(), entry)
        };
        tx.journal_release_run(r.id, &entry)?;
        let plan = releaser::Plan {
            release: r.id,
            project: project.id,
            repo,
            base: project.work_branch.clone(),
            tag: release_run::tag_name(&version),
            version,
            check: project.check_command.clone(),
            bump: project.bump_command.clone(),
            artifact: project.artifact_command.clone(),
            remote,
            steps,
            todo,
            resuming: input.resume,
            scratch: PathBuf::new(),
        };
        let chain_id = r.chain_id;
        *slot
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(plan);
        Ok(chain_id)
    })
    .await?;
    let plan = started
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take();
    if let Some(mut plan) = plan {
        plan.scratch = state.ships.join(".releases");
        tokio::spawn(release_task::run_release(state, plan));
    }
    Ok(Json(detail))
}

#[utoipa::path(get, path = "/releases/{release}/run", tag = "releases",
    params(("release" = String, Path, description = "Release id or version")),
    responses((status = 200, body = ReleaseRun), (status = 404, body = ErrorBody)))]
pub async fn release_run(
    State(state): State<AppState>,
    Path(release): Path<String>,
) -> ApiResult<Json<ReleaseRun>> {
    let checks = state.store.check_line().standing();
    let run = read(&state, move |repo| {
        let r = resolve::release(repo, &release)?;
        let run = repo
            .release_run(r.id)?
            .ok_or_else(|| AppError::not_found(format!("a release run of {}", r.version)))?;
        release_task::with_waits(repo, &checks, r.id, run)
    })
    .await?;
    Ok(Json(run))
}

#[utoipa::path(get, path = "/projects/{project}/next-version", tag = "releases",
    params(("project" = String, Path, description = "Project id or key")),
    responses((status = 200, body = VersionSuggestion), (status = 404, body = ErrorBody)))]
/// The version a new chain of the project suggests: the next patch after the greatest of the
/// repository's newest version tag and the versions the project's chains plan, or `0.1.0` when
/// there is none.
pub async fn next_version(
    State(state): State<AppState>,
    Path(project): Path<String>,
) -> ApiResult<Json<VersionSuggestion>> {
    let (project, planned) = read(&state, move |repo| {
        let p = resolve::project(repo, &project)?;
        let planned: Vec<String> = repo
            .chains(Some(p.id))?
            .into_iter()
            .filter_map(|c| c.version)
            .collect();
        Ok((p, planned))
    })
    .await?;
    let tags = match project.repo_path.filter(|r| !r.trim().is_empty()) {
        Some(repo) => tokio::task::spawn_blocking(move || releaser::tags(&repo))
            .await
            .unwrap_or_default(),
        None => Vec::new(),
    };
    Ok(Json(VersionSuggestion {
        version: toknila_core::roadmap::next_version(&tags, &planned),
        last_tag: tags.first().cloned(),
    }))
}
