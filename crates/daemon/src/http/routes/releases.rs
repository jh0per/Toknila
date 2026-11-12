//! `/chains/{chain}/releases`, `/releases`.

use std::path::PathBuf;

use crate::http::body::Json;
use axum::extract::{Path, State};
use toknila_app::releases::{self, ReleaseAction};
use toknila_app::{AppError, resolve};
use toknila_core::{Actor, release_run};

use super::{ApiResult, chain_write, read};
use crate::http::dto::{
    ChainDetail, NewRelease, ReleaseOffer, ReleaseRun, StartRelease, VersionSuggestion,
};
use crate::http::error::ErrorBody;
use crate::releaser;
use crate::server::{AppState, now};
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

/// What the Release dialog needs: the version to suggest and the steps the project has.
fn project_steps(p: &toknila_core::Project) -> Vec<toknila_core::release_run::ReleaseStep> {
    toknila_core::release_run::steps(
        p.check_command.as_deref(),
        p.bump_command.as_deref(),
        p.merge.mode == toknila_core::MergeMode::Github,
    )
}

fn repo_of(p: &toknila_core::Project) -> Result<String, AppError> {
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
        let run = match end {
            Some(r) => repo.release_run(r.id)?,
            None => None,
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
    Ok(Json(ReleaseOffer {
        releasable,
        version: own.unwrap_or_else(|| release_run::suggest_version(&tags)),
        last_tag: tags.first().cloned(),
        steps: project_steps(&project),
        base: project.work_branch.clone(),
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
        let steps = project_steps(&project);
        let last = tx.repo().release_run(r.id)?;
        if last.as_ref().is_some_and(|l| l.state == RunState::Running) {
            return Err(AppError::conflict("a release of this chain is already running").into());
        }
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
            repo,
            base: project.work_branch.clone(),
            tag: release_run::tag_name(&version),
            version,
            check: project.check_command.clone(),
            bump: project.bump_command.clone(),
            push: project.merge.mode == toknila_core::MergeMode::Github,
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
        tokio::spawn(run_release(state, plan));
    }
    Ok(Json(detail))
}

/// Journals `entry` of the run of `id`; blocking, for the run's thread.
fn journal(
    store: &crate::store::Store,
    handle: &tokio::runtime::Handle,
    id: toknila_core::ReleaseId,
    entry: Entry,
) -> Result<(), String> {
    handle
        .block_on(store.write(Actor::System, now(), move |tx| {
            tx.journal_release_run(id, &entry)
        }))
        .map(|_: ((), _)| ())
        .map_err(|e: crate::store::StoreError| e.to_string())
}

/// The run itself: the steps on a blocking thread, then the milestone is saved as released and
/// the run ends in the same transaction.
async fn run_release(state: AppState, plan: releaser::Plan) {
    let (id, tag) = (plan.release, plan.tag.clone());
    let ran = {
        let plan = plan.clone();
        let store = state.store.clone();
        let handle = tokio::runtime::Handle::current();
        tokio::task::spawn_blocking(move || {
            releaser::execute(&plan, &|e| journal(&store, &handle, id, e))
        })
        .await
    };
    let failure = match ran {
        Ok(Ok(())) => {
            let saved = state
                .store
                .write(Actor::Human, now(), move |tx| -> ApiResult<()> {
                    releases::finish(tx, id, &tag)?;
                    Ok(tx.journal_release_run(id, &Entry::RunDone)?)
                })
                .await;
            match saved {
                Ok(_) => return,
                Err(e) => format!("the release could not be saved: {e:?}"),
            }
        }
        Ok(Err(_)) => return,
        Err(e) => format!("the release run crashed: {e}"),
    };
    let entry = Entry::RunFailed { error: failure };
    let _ = state
        .store
        .write(Actor::System, now(), move |tx| {
            tx.journal_release_run(id, &entry)
        })
        .await
        .map(|_: ((), _)| ())
        .map_err(|e: crate::store::StoreError| e);
}

#[utoipa::path(get, path = "/releases/{release}/run", tag = "releases",
    params(("release" = String, Path, description = "Release id or version")),
    responses((status = 200, body = ReleaseRun), (status = 404, body = ErrorBody)))]
pub async fn release_run(
    State(state): State<AppState>,
    Path(release): Path<String>,
) -> ApiResult<Json<ReleaseRun>> {
    let run = read(&state, move |repo| {
        let r = resolve::release(repo, &release)?;
        repo.release_run(r.id)?
            .ok_or_else(|| AppError::not_found(format!("a release run of {}", r.version)).into())
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
