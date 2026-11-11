//! `/chains/{chain}/releases`, `/releases`.

use std::path::PathBuf;

use axum::Json;
use axum::extract::{Path, State};
use toknila_app::releases::{self, ReleaseAction};
use toknila_app::{AppError, resolve};
use toknila_core::{Actor, release_run};

use super::{ApiResult, chain_write, read};
use crate::http::dto::{ChainDetail, NewRelease, ReleaseOffer, ReleaseRun, StartRelease};
use crate::http::error::ErrorBody;
use crate::releaser;
use crate::server::{AppState, now};

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
    let (releasable, own, project) = read(&state, move |repo| {
        let c = resolve::chain(repo, &chain)?;
        let view = repo
            .chain_view(c.id)?
            .ok_or_else(|| AppError::not_found("chain"))?;
        let project = repo
            .project(c.project_id)?
            .ok_or_else(|| AppError::not_found("project"))?;
        let own = view.end_release().map(|r| r.version.clone());
        Ok((view.releasable(), own, project))
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
        let plan = releaser::Plan {
            release: r.id,
            repo,
            base: project.work_branch.clone(),
            tag: release_run::tag_name(&version),
            version,
            check: project.check_command.clone(),
            bump: project.bump_command.clone(),
            push: project.merge.mode == toknila_core::MergeMode::Github,
            steps: project_steps(&project),
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
        if !releaser::begin(&plan) {
            return Err(AppError::conflict("a release of this chain is already running").into());
        }
        tokio::spawn(run_release(state, plan));
    }
    Ok(Json(detail))
}

/// The run itself: the steps on a blocking thread, then the milestone is saved as released.
async fn run_release(state: AppState, plan: releaser::Plan) {
    let (id, tag) = (plan.release, plan.tag.clone());
    let ran = {
        let plan = plan.clone();
        tokio::task::spawn_blocking(move || releaser::execute(&plan)).await
    };
    match ran {
        Ok(Ok(())) => {
            let saved = state
                .store
                .write(Actor::Human, now(), move |tx| -> ApiResult<()> {
                    Ok(releases::finish(tx, id, &tag)?)
                })
                .await;
            match saved {
                Ok(_) => releaser::done(id),
                Err(e) => releaser::failed(id, format!("the release could not be saved: {e:?}")),
            }
        }
        Ok(Err(_)) => {}
        Err(e) => releaser::failed(id, format!("the release run crashed: {e}")),
    }
}

#[utoipa::path(get, path = "/releases/{release}/run", tag = "releases",
    params(("release" = String, Path, description = "Release id or version")),
    responses((status = 200, body = ReleaseRun), (status = 404, body = ErrorBody)))]
pub async fn release_run(
    State(state): State<AppState>,
    Path(release): Path<String>,
) -> ApiResult<Json<ReleaseRun>> {
    let r = read(&state, move |repo| Ok(resolve::release(repo, &release)?)).await?;
    let run = releaser::run_of(r.id)
        .ok_or_else(|| AppError::not_found(format!("a release run of {}", r.version)))?;
    Ok(Json(run))
}
