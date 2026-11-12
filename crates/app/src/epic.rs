//! The chain as an epic: its name, goal and version, and the end release that carries the
//! version; the settings a person changes after a chain is made.

use serde::{Deserialize, Serialize};
use toknila_core::{
    ChainId, Release, ReleaseChannel, ReleaseId, ReleaseState, ReviewMode, StartMode,
};

use crate::chains::clean_version;
use crate::{AppError, AppResult, ChainTx, UnitOfWork, resolve, with_chain};

/// Sets the chain's version while nothing is released: the end release is renamed, or made, so
/// the chain and its end release always say the same version. A release that was cut or released
/// keeps its version, and the refusal says why.
pub(crate) fn set_version(ctx: &mut ChainTx, text: &str) -> AppResult<()> {
    let version = clean_version(text)?;
    let end = ctx.view.finish_release().cloned();
    let end_id = end.as_ref().map(|r| r.id);
    let clash = ctx
        .view
        .releases
        .iter()
        .any(|r| Some(r.id) != end_id && r.version == version);
    if let Some(r) = &end
        && r.state != ReleaseState::Planned
    {
        let why = if r.state == ReleaseState::Cut {
            "being released"
        } else {
            "released"
        };
        return Err(AppError::conflict(format!(
            "release {} is {why}: its version cannot change",
            r.version
        )));
    }
    if clash {
        return Err(AppError::conflict(format!(
            "the epic already has a release {version}"
        )));
    }
    match end {
        Some(r) if r.version != version => ctx.set_release_version(r.id, version.clone())?,
        Some(_) => {}
        None => {
            let release = Release::planned(
                ReleaseId::new(),
                ctx.view.chain.id,
                version.clone(),
                ReleaseChannel::TagOnly,
                StartMode::Manual,
            );
            let id = release.id;
            ctx.add_release(release);
            ctx.wire_end_release(id)?;
        }
    }
    ctx.edit_chain(|c| c.version = Some(version));
    Ok(())
}

/// What a person may set on a chain after it is made (a project takes the review fields in
/// [`crate::projects::PatchProject`]). A field left out is kept.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct ChainSettings {
    pub review_mode: Option<ReviewMode>,
    /// Reviewer agent profile; an empty text goes back to `<worker profile>-reviewer`.
    pub reviewer: Option<String>,
    /// Whether the chain starts its ready cards by itself. Turning it on starts the ready cards
    /// that have a free slot, so it is a person's act; the planner cannot change a chain's graph
    /// while it is on.
    pub autostart: Option<bool>,
    /// The epic's name; unique in the project.
    pub name: Option<String>,
    /// The epic's goal, markdown: what every card of the chain works toward.
    pub goal: Option<String>,
    /// The version the chain releases; refused once its release was cut.
    pub version: Option<String>,
}

/// Sets a chain's name, goal, version, review mode, reviewer and autostart, journals it
/// (`ChainUpdated`) and lets the schedule run, since turning autostart on may start cards.
/// `project` narrows the name lookup. Returns the chain's id.
pub fn set_settings<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    chain: &str,
    project: Option<&str>,
    settings: ChainSettings,
) -> AppResult<ChainId> {
    let c = resolve::chain_in(uow, project, chain)?;
    let name = match settings.name.as_deref().map(str::trim) {
        Some("") => return Err(AppError::bad_request("epic name is empty")),
        Some(n) if n != c.name => {
            if uow.chain_by_name(c.project_id, n)?.is_some() {
                return Err(AppError::conflict(format!("epic {n} already exists")));
            }
            Some(n.to_owned())
        }
        _ => None,
    };
    with_chain(uow, c.id, |ctx| -> AppResult<()> {
        if let Some(v) = settings.version.as_deref().filter(|v| !v.trim().is_empty())
            && ctx.view.version().as_deref() != Some(v.trim().trim_start_matches('v'))
        {
            crate::epic::set_version(ctx, v)?;
        }
        ctx.edit_chain(|c| {
            if let Some(n) = name {
                c.name = n;
            }
            if let Some(g) = settings.goal {
                c.goal = g.trim().to_owned();
            }
            if let Some(m) = settings.review_mode {
                c.review_mode = Some(m);
            }
            if let Some(r) = settings.reviewer {
                c.reviewer = Some(r.trim().to_owned()).filter(|r| !r.is_empty());
            }
            if let Some(a) = settings.autostart {
                c.autostart = a;
            }
        });
        Ok(())
    })?;
    Ok(c.id)
}
