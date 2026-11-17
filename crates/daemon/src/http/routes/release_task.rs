//! The release run in the background, after the person's click on Release: it takes its turn in
//! the project's merge line, runs its steps, and journals each one; every entry lets the merge
//! line move on (a Tag or Push that ended, or a failure, frees the turn for the next card). And
//! what a run shown to a person waits for.

use std::time::Duration;

use toknila_api_types::{ReleaseRun, RunState, StepState};
use toknila_app::{AppError, merges, releases, resolve};
use toknila_core::release_run::{ReleaseStep, check_line_words, command_words};
use toknila_core::{Actor, ProjectId, ReleaseId};

use super::{ApiResult, read};
use crate::http::error::ApiError;
use crate::releaser;
use crate::server::{AppState, now};
use crate::store::Repo;
use crate::store::release_runs::Entry;

/// How often a run that waits for its turn in the merge line asks again.
const TURN_POLL: Duration = Duration::from_millis(250);

/// Journals `entry` of the run of `release` and lets `project`'s merge line move on, in one
/// transaction.
fn record(
    tx: &mut crate::store::WriteTx<'_>,
    (release, project): (ReleaseId, ProjectId),
    entry: &Entry,
) -> Result<(), ApiError> {
    tx.journal_release_run(release, entry)?;
    merges::advance(tx, project)?;
    Ok(())
}

/// Journals `entry` of the run; blocking, for the run's thread.
fn journal(
    store: &crate::store::Store,
    handle: &tokio::runtime::Handle,
    run: (ReleaseId, ProjectId),
    entry: Entry,
) -> Result<(), String> {
    handle
        .block_on(store.write(Actor::System, now(), move |tx| record(tx, run, &entry)))
        .map(|((), _)| ())
        .map_err(|e| e.to_string())
}

/// Waits until the run holds its project's merge line: the turn in progress (a card's sync,
/// re-check and merge) ends first; the cards that wait go after the run.
async fn take_turn(state: &AppState, (release, project): (ReleaseId, ProjectId)) -> ApiResult<()> {
    loop {
        let (waits, _) = state
            .store
            .write(Actor::System, now(), move |tx| -> ApiResult<_> {
                Ok(merges::take_release_turn(tx, project, release)?)
            })
            .await?;
        if waits.is_none() {
            return Ok(());
        }
        tokio::time::sleep(TURN_POLL).await;
    }
}

/// The finished epics of the project whose merges the new tag contains: they ship with it. A
/// failure to look means none are carried; the release itself stands.
async fn carried_by(state: &AppState, plan: &releaser::Plan) -> Vec<toknila_core::ChainId> {
    let id = plan.release;
    let epics = read(state, move |repo| {
        let r = resolve::release(repo, &id.to_string())?;
        let c = repo
            .chain(r.chain_id)?
            .ok_or_else(|| AppError::not_found("epic"))?;
        Ok(releases::finished_epics(repo, c.project_id, Some(c.id))?)
    })
    .await
    .unwrap_or_default();
    if epics.is_empty() {
        return Vec::new();
    }
    let (repo, tag) = (plan.repo.clone(), plan.tag.clone());
    tokio::task::spawn_blocking(move || {
        let (_, commit) = crate::release_carry::tag_commit(&repo, &tag)?;
        crate::release_carry::contained(&repo, &commit, &epics)
    })
    .await
    .ok()
    .and_then(Result::ok)
    .unwrap_or_default()
}

/// The run itself: its turn in the merge line, the steps on a blocking thread, then the
/// milestone is saved as released and the run ends in the same transaction.
pub(super) async fn run_release(state: AppState, plan: releaser::Plan) {
    let (id, tag) = (plan.release, plan.tag.clone());
    let run = (id, plan.project);
    if let Err(e) = take_turn(&state, run).await {
        return failed(
            &state,
            run,
            format!("the release could not take its turn: {e}"),
        )
        .await;
    }
    let ran = {
        let plan = plan.clone();
        let store = state.store.clone();
        let line = state.store.check_line();
        let handle = tokio::runtime::Handle::current();
        tokio::task::spawn_blocking(move || {
            releaser::execute(&plan, &|e| journal(&store, &handle, run, e), &line)
        })
        .await
    };
    let failure = match ran {
        Ok(Ok(())) => {
            let carried = carried_by(&state, &plan).await;
            let saved = state
                .store
                .write(Actor::Human, now(), move |tx| -> ApiResult<()> {
                    releases::finish(tx, id, &tag, &carried)?;
                    record(tx, run, &Entry::RunDone)
                })
                .await;
            match saved {
                Ok(_) => return,
                Err(e) => format!("the release could not be saved: {e:?}"),
            }
        }
        // The failed step is journaled.
        Ok(Err(_)) => return,
        Err(e) => format!("the release run crashed: {e}"),
    };
    failed(&state, run, failure).await;
}

/// The run failed with no step to blame; its turn in the merge line ends with it.
async fn failed(state: &AppState, run: (ReleaseId, ProjectId), error: String) {
    let entry = Entry::RunFailed { error };
    let _ = state
        .store
        .write(Actor::System, now(), move |tx| record(tx, run, &entry))
        .await;
}

/// The Build step reads with the first words of the project's artifact command (`Build:
/// ./release-mac.sh`), so a person sees what runs.
fn name_build(repo: &Repo<'_>, release: ReleaseId, run: &mut ReleaseRun) -> ApiResult<()> {
    if !run.steps.iter().any(|s| s.step == ReleaseStep::Build) {
        return Ok(());
    }
    let Some(r) = repo.release(release)? else {
        return Ok(());
    };
    let Some(project) = repo
        .chain(r.chain_id)?
        .and_then(|c| repo.project(c.project_id).ok().flatten())
    else {
        return Ok(());
    };
    let words = project
        .artifact_command
        .as_deref()
        .filter(|c| !c.trim().is_empty())
        .map(command_words);
    if let (Some(words), Some(step)) = (
        words,
        run.steps.iter_mut().find(|s| s.step == ReleaseStep::Build),
    ) {
        step.label = format!("{}: {words}", step.step.label());
    }
    Ok(())
}

/// `run` as a person reads it while it goes: the step that cannot start says what it waits
/// for, its run's turn in the merge line or its Check's turn in the machine's check line.
pub(super) fn with_waits(
    repo: &Repo<'_>,
    checks: &toknila_app::checks::Standing,
    release: ReleaseId,
    mut run: ReleaseRun,
) -> ApiResult<ReleaseRun> {
    name_build(repo, release, &mut run)?;
    if run.state != RunState::Running {
        return Ok(run);
    }
    if !run.turn {
        let project = match repo.release(release)? {
            Some(r) => repo.chain(r.chain_id)?.map(|c| c.project_id),
            None => None,
        };
        let wait = match project {
            Some(p) => merges::line_of(repo, p)?.release_wait(release),
            None => None,
        };
        if let Some(step) = run.steps.iter_mut().find(|s| s.state != StepState::Done) {
            step.wait = wait.map(|w| w.words());
        }
        return Ok(run);
    }
    let check = run.steps.iter_mut().find(|s| {
        matches!(s.step, ReleaseStep::Check | ReleaseStep::Build) && s.state == StepState::Running
    });
    if let Some(step) = check
        && let Some((place, held)) = checks.place_release(release).filter(|(n, _)| *n > 0)
    {
        step.wait = Some(check_line_words(place, held));
    }
    Ok(run)
}
