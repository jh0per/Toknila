//! Milestones: create them in a chain, cut and complete them.

use std::str::FromStr;

use serde::{Deserialize, Serialize};
use toknila_core::graph::ChainView;
use toknila_core::{
    ChainId, Edge, EdgeKind, NodeRef, ProjectId, Release, ReleaseChannel, ReleaseId, ReleaseState,
    StartMode,
};

use crate::chains::change;
use crate::{AppError, AppResult, Reader, UnitOfWork, with_chain};

/// `POST /v1/chains/{chain}/releases`.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NewRelease {
    pub version: String,
    /// Defaults to tag_only.
    pub channel: Option<ReleaseChannel>,
    /// Defaults to manual.
    pub start_mode: Option<StartMode>,
    /// Nodes the milestone waits for (ticket keys or versions).
    #[serde(default)]
    pub after: Vec<String>,
    /// Nodes that wait for the milestone.
    #[serde(default)]
    pub before: Vec<String>,
}

/// Adds a planned milestone to a chain with its dependencies.
pub fn create<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    chain: &str,
    input: NewRelease,
) -> AppResult<ChainId> {
    let version = input.version.trim().to_owned();
    if version.is_empty() {
        return Err(AppError::bad_request("release version is empty"));
    }
    change(uow, chain, move |ctx| {
        if ctx.view.releases.iter().any(|r| r.version == input.version) {
            return Err(AppError::conflict(format!(
                "release {} already exists",
                input.version
            )));
        }
        let chain_id = ctx.view.chain.id;
        let r = Release::planned(
            ReleaseId::new(),
            chain_id,
            version,
            input.channel.unwrap_or_default(),
            input.start_mode.unwrap_or_default(),
        );
        let node = NodeRef::Release(r.id);
        ctx.add_release(r);
        let edge = |from, to| Edge {
            chain_id,
            from,
            to,
            kind: EdgeKind::Normal,
        };
        for a in &input.after {
            let from = crate::resolve::node(&ctx.view, a)?;
            ctx.add_edge(edge(from, node))?;
        }
        for b in &input.before {
            let to = crate::resolve::node(&ctx.view, b)?;
            ctx.add_edge(edge(node, to))?;
        }
        Ok(())
    })
}

/// What a person can do with a milestone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReleaseAction {
    /// Cut it: its tickets are frozen into it.
    Start,
    /// Mark it released.
    Complete,
}

impl FromStr for ReleaseAction {
    type Err = AppError;

    fn from_str(s: &str) -> AppResult<Self> {
        match s {
            "start" => Ok(Self::Start),
            "complete" => Ok(Self::Complete),
            other => Err(AppError::not_found(format!("action {other}"))),
        }
    }
}

/// Applies a person's action to a milestone.
pub fn act<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    r: &Release,
    action: ReleaseAction,
) -> AppResult<()> {
    with_chain(uow, r.chain_id, |ctx| -> AppResult<()> {
        match action {
            ReleaseAction::Start => ctx.cut_release(r.id)?,
            ReleaseAction::Complete => ctx.complete_release(r.id, None)?,
        }
        Ok(())
    })
}

/// Begins a release of the whole chain at `version`: every card must be in the work branch. The
/// chain's end milestone is used and cut when it is only planned; a chain without one gets a
/// milestone that waits for its last cards. Returns the cut milestone and its project. Nothing
/// is tagged or pushed here: that is the release run's work.
pub fn begin<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    chain: &str,
    version: &str,
) -> AppResult<(Release, toknila_core::Project)> {
    let version =
        toknila_core::release_run::clean_version(version).map_err(AppError::bad_request)?;
    let c = crate::resolve::chain(uow, chain)?;
    let project = uow
        .project(c.project_id)?
        .ok_or_else(|| AppError::not_found("project"))?;
    let release = with_chain(uow, c.id, |ctx| -> AppResult<Release> {
        if !ctx.view.releasable() {
            return Err(AppError::conflict(
                "the epic still has open stories: release it when every story is merged",
            ));
        }
        let existing = ctx.view.end_release().cloned();
        let id = match existing {
            Some(r) if r.version != version => {
                return Err(AppError::bad_request(format!(
                    "the epic's release is {}, not {version}",
                    r.version
                )));
            }
            Some(r) => {
                // Cards added since the milestone was made are before it too.
                ctx.wire_end_release(r.id)?;
                r.id
            }
            None => {
                if ctx.view.releases.iter().any(|r| r.version == version) {
                    return Err(AppError::conflict(format!(
                        "release {version} already exists"
                    )));
                }
                let chain_id = ctx.view.chain.id;
                let r = Release::planned(
                    ReleaseId::new(),
                    chain_id,
                    version.clone(),
                    ReleaseChannel::TagOnly,
                    StartMode::Manual,
                );
                let id = r.id;
                let leaves = ctx.view.leaf_tickets();
                ctx.add_release(r);
                for t in leaves {
                    ctx.add_edge(Edge {
                        chain_id,
                        from: NodeRef::Ticket(t),
                        to: NodeRef::Release(id),
                        kind: EdgeKind::Normal,
                    })?;
                }
                id
            }
        };
        if ctx
            .view
            .releases
            .iter()
            .any(|r| r.id == id && r.state == ReleaseState::Planned)
        {
            ctx.cut_release(id)?;
        }
        ctx.view
            .releases
            .iter()
            .find(|r| r.id == id)
            .cloned()
            .ok_or_else(|| AppError::not_found("release"))
    })?;
    Ok((release, project))
}

/// Marks a cut milestone released with the tag its release run made, and every epic in `carried`
/// (the finished epics the tag contains, see [`finished_epics`]) released with it.
pub fn finish<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    release: ReleaseId,
    tag: &str,
    carried: &[ChainId],
) -> AppResult<()> {
    let r = uow
        .release(release)?
        .ok_or_else(|| AppError::not_found("release"))?;
    let by = uow.chain(r.chain_id)?.map(|c| c.name);
    with_chain(uow, r.chain_id, |ctx| -> AppResult<()> {
        ctx.complete_release(release, Some(tag.to_owned()))?;
        Ok(())
    })?;
    for chain in carried {
        // An epic that changed since the run looked (a story reopened, released by hand) stays.
        let still = uow.chain_view(*chain)?.is_some_and(|v| ships_in_a_tag(&v));
        if still {
            mark_with(uow, *chain, tag, by.as_deref())?;
        }
    }
    Ok(())
}

/// A finished epic of a project, as a tag has to contain it to carry it.
#[derive(Debug, Clone)]
pub struct FinishedEpic {
    pub chain: ChainId,
    pub name: String,
    /// The keys of its Merged stories.
    pub keys: Vec<String>,
}

fn ships_in_a_tag(v: &ChainView) -> bool {
    v.carriable()
        && v.finish_release()
            .is_some_and(|r| r.state == ReleaseState::Planned)
}

impl FinishedEpic {
    /// The epic of `view` when it can ship inside a tag: every story Merged, none Released or
    /// open, and a planned release node to mark.
    pub fn of(view: &ChainView) -> Option<Self> {
        ships_in_a_tag(view).then(|| Self {
            chain: view.chain.id,
            name: view.chain.name.clone(),
            keys: view
                .tickets
                .iter()
                .filter(|t| t.in_plan() && t.status == toknila_core::TicketStatus::Merged)
                .map(|t| t.key.to_string())
                .collect(),
        })
    }
}

/// The finished epics of a project, but `except`: what a tag of a release run may carry.
pub fn finished_epics<R: Reader + ?Sized>(
    repo: &R,
    project: ProjectId,
    except: Option<ChainId>,
) -> AppResult<Vec<FinishedEpic>> {
    let mut found = Vec::new();
    for c in repo.chains(Some(project))? {
        if Some(c.id) != except
            && let Some(view) = repo.chain_view(c.id)?
            && let Some(epic) = FinishedEpic::of(&view)
        {
            found.push(epic);
        }
    }
    Ok(found)
}

/// The epic of the project whose release run made `tag`, by name.
pub fn cutter_of<R: Reader + ?Sized>(
    repo: &R,
    project: ProjectId,
    tag: &str,
) -> AppResult<Option<String>> {
    for c in repo.chains(Some(project))? {
        if let Some(view) = repo.chain_view(c.id)?
            && view
                .releases
                .iter()
                .any(|r| r.tag.as_deref() == Some(tag) && r.released_with.is_none())
        {
            return Ok(Some(c.name));
        }
    }
    Ok(None)
}

/// Marks the epic's planned release released with `tag`, named by the epic that cut it when there
/// is one. The caller checked that the tag contains the epic's merges.
pub fn mark_with<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    chain: ChainId,
    tag: &str,
    by: Option<&str>,
) -> AppResult<()> {
    with_chain(uow, chain, |ctx| -> AppResult<()> {
        ctx.release_with(tag, by)?;
        Ok(())
    })
}

/// Refuses `version` when another unreleased epic of the project plans it. `except` is the epic
/// being saved.
pub fn ensure_version_free<R: Reader + ?Sized>(
    repo: &R,
    project: ProjectId,
    except: Option<ChainId>,
    version: &str,
) -> AppResult<()> {
    for c in repo.chains(Some(project))? {
        if Some(c.id) == except {
            continue;
        }
        let Some(view) = repo.chain_view(c.id)? else {
            continue;
        };
        let released = view
            .finish_release()
            .is_some_and(|r| r.state == ReleaseState::Released);
        if !released && view.version().as_deref() == Some(version) {
            return Err(AppError::conflict(format!(
                "{version} is planned by {}; pick another version or move that one",
                c.name
            )));
        }
    }
    Ok(())
}
