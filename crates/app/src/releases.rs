//! Milestones: create them in a chain, cut and complete them.

use std::str::FromStr;

use serde::{Deserialize, Serialize};
use toknila_core::{
    ChainId, Edge, EdgeKind, NodeRef, Release, ReleaseChannel, ReleaseId, ReleaseState, StartMode,
};

use crate::chains::change;
use crate::{AppError, AppResult, UnitOfWork, with_chain};

/// `POST /v1/chains/{chain}/releases`.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
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
                "the chain still has open cards: release it when every card is merged",
            ));
        }
        let existing = ctx.view.end_release().cloned();
        let id = match existing {
            Some(r) if r.version != version => {
                return Err(AppError::bad_request(format!(
                    "the chain's release is {}, not {version}",
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

/// Marks a cut milestone released with the tag its release run made.
pub fn finish<U: UnitOfWork + ?Sized>(uow: &mut U, release: ReleaseId, tag: &str) -> AppResult<()> {
    let r = uow
        .release(release)?
        .ok_or_else(|| AppError::not_found("release"))?;
    with_chain(uow, r.chain_id, |ctx| -> AppResult<()> {
        ctx.complete_release(release, Some(tag.to_owned()))?;
        Ok(())
    })
}
