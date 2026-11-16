//! The plan's commands as a caller makes them, whoever it is: the access rule first
//! ([`crate::access`], core's `access::may`), then the use case the window and the CLI have always
//! used, then, for the assistant, a line in its project's conversation that says what it did. The
//! person's routes call these with the person and the planner's routes with its planner, so one
//! route serves one action and no handler holds a rule.

mod stories;

pub use stories::{add_story, edit_story, open_thread, show_story, thread};

use toknila_core::conversation::{self, About};
use toknila_core::graph::Place;
use toknila_core::{Chain, ChainId, EdgeKind};

use crate::access::{self, Action, Caller};
use crate::chains::{self, ChainSettings, NewChain};
use crate::{AppError, AppResult, Reader, UnitOfWork, moves, resolve};

/// A new epic at the end of `project`'s roadmap (id or key). A planner's never starts its stories
/// by itself, and its review settings are the owner's to set.
pub fn new_epic<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    caller: &Caller,
    project: &str,
    mut input: NewChain,
) -> AppResult<ChainId> {
    let p = resolve::project(&*uow, project)?;
    let asked = Action::NewEpic {
        autostart: input.autostart,
        owner_fields: input.review_mode.is_some() || input.reviewer.is_some(),
    };
    access::on_project(&*uow, caller, &asked, p.id)?;
    input.autostart = Some(toknila_core::access::new_epic_autostart(
        caller,
        input.autostart,
    ));
    let id = chains::create(uow, &p.id.to_string(), input)?;
    if let Some(c) = uow.chain(id)? {
        let version = uow
            .chain_view(id)?
            .and_then(|v| v.finish_release().map(|r| r.version.clone()));
        let text = conversation::added_chain(&c.display_title(), version.as_deref());
        access::tell(uow, caller, text, Some(About::chain_of(&c)))?;
    }
    Ok(id)
}

/// Changes an epic's settings; a field left out is kept. `project` (id or key) narrows a
/// person's lookup of the name. A planner changes only the name, title, goal and version of an
/// epic whose release is not cut.
pub fn edit_epic<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    caller: &Caller,
    chain: &str,
    project: Option<&str>,
    settings: ChainSettings,
) -> AppResult<ChainId> {
    let c = access::chain(&*uow, caller, project, chain)?;
    let owner_fields = settings.autostart.is_some()
        || settings.review_mode.is_some()
        || settings.reviewer.is_some();
    access::on_epic(&*uow, caller, &Action::EditEpic { owner_fields }, &c)?;
    let id = chains::set_settings(uow, &c.id.to_string(), None, settings)?;
    // The epic as it is now: the edit may have renamed it.
    let now = uow.chain(id)?.unwrap_or(c);
    let text = conversation::edited_chain(&now.display_title());
    access::tell(uow, caller, text, Some(About::chain_of(&now)))?;
    Ok(id)
}

/// The epic `chain` names, once `caller` may change its graph (`owner_fields`: a dependency that
/// waits for a release).
fn graph_epic<R: Reader + ?Sized>(
    r: &R,
    caller: &Caller,
    chain: &str,
    owner_fields: bool,
) -> AppResult<Chain> {
    let c = access::chain(r, caller, None, chain)?;
    access::on_epic(r, caller, &Action::ChangeGraph { owner_fields }, &c)?;
    Ok(c)
}

/// Makes `to` wait for `from` in an epic.
pub fn add_dependency<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    caller: &Caller,
    chain: &str,
    (from, to): (&str, &str),
    kind: EdgeKind,
) -> AppResult<ChainId> {
    let c = graph_epic(&*uow, caller, chain, kind != EdgeKind::Normal)?;
    chains::add_edge(uow, &c.id.to_string(), from, to, kind)
}

/// Removes the wait of `to` for `from`.
pub fn remove_dependency<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    caller: &Caller,
    chain: &str,
    (from, to): (&str, &str),
) -> AppResult<ChainId> {
    let c = graph_epic(&*uow, caller, chain, false)?;
    chains::remove_edge(uow, &c.id.to_string(), from, to)
}

/// Puts a story directly before or after `anchor` in its epic; the dependencies around it are
/// rewired.
pub fn move_story<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    caller: &Caller,
    chain: &str,
    story: &str,
    (anchor, place): (&str, Place),
) -> AppResult<ChainId> {
    let c = graph_epic(&*uow, caller, chain, false)?;
    chains::move_ticket(uow, &c.id.to_string(), story, anchor, place)
}

/// Moves a story that has not started to another epic of its project, to the end or next to
/// `place`'s anchor. `from`, when given, is the epic the caller says the story is in. When a
/// planner moves it, neither epic may start its stories by itself. Returns the receiving epic.
pub fn move_story_to_epic<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    caller: &Caller,
    story: &str,
    to: &str,
    place: Option<(&str, Place)>,
    from: Option<&str>,
) -> AppResult<ChainId> {
    let read = Action::ReadPlan;
    let named = from
        .map(|f| access::chain(&*uow, caller, None, f))
        .transpose()?;
    if let Some(f) = &named {
        access::on_epic(&*uow, caller, &read, f)?;
    }
    let t = resolve::ticket(&*uow, story)?;
    let own = uow
        .chain(t.chain_id)?
        .ok_or_else(|| AppError::not_found(format!("epic of {}", t.key)))?;
    let target = moves::target_epic(&*uow, &own, to)?;
    access::on_epic(&*uow, caller, &read, &target)?;
    let from = named.unwrap_or(own);
    let change = Action::ChangeGraph {
        owner_fields: false,
    };
    access::on_epic(&*uow, caller, &change, &from)?;
    access::on_epic(&*uow, caller, &change, &target)?;
    access::on_story(&*uow, caller, &read, &t)?;
    if t.chain_id != from.id {
        return Err(AppError::bad_request(format!(
            "{} is not in epic {}",
            t.key, from.name
        )));
    }
    moves::move_to_chain(uow, &t, &target.id.to_string(), place)?;
    Ok(target.id)
}
