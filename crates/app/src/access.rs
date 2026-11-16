//! The application's side of [`toknila_core::access`]: it finds what a call is on, reads what the
//! rule looks at (the project's key, the epic's release, the conversation turn) and asks core
//! whether the caller may. Every use case a token other than the person's reaches goes through
//! here; no handler holds a rule of its own.

use toknila_core::access::{self, Refusal, Target};
use toknila_core::conversation::{About, Author, Message};
use toknila_core::{Chain, ChainId, ProjectId, ReleaseState, Ticket};

pub use toknila_core::access::{Action, Caller};

use crate::{AppError, AppResult, ErrorKind, Reader, UnitOfWork, resolve};

impl From<Refusal> for AppError {
    fn from(r: Refusal) -> Self {
        let kind = if r.is_conflict() {
            ErrorKind::Conflict
        } else {
            ErrorKind::Forbidden
        };
        AppError::new(kind, r.to_string())
    }
}

/// A chain by id or name as `caller` names it. A planner's names are looked up in its own project
/// first, and a name only another project has is that project's epic (which [`on_epic`] then
/// refuses it); anyone else names a chain by id or by a name unique in `project` (id or key) when
/// it is given, in every project otherwise.
pub fn chain<R: Reader + ?Sized>(
    r: &R,
    caller: &Caller,
    project: Option<&str>,
    reference: &str,
) -> AppResult<Chain> {
    let Some(project) = caller.planner_project() else {
        return resolve::chain_in(r, project, reference);
    };
    if let Ok(id) = reference.parse::<ChainId>()
        && let Some(c) = r.chain(id)?
    {
        return Ok(c);
    }
    if let Some(c) = r.chain_by_name(project, reference)? {
        return Ok(c);
    }
    r.chains_named(reference)?
        .into_iter()
        .next()
        .ok_or_else(|| AppError::not_found(format!("epic {reference}")))
}

fn key_of<R: Reader + ?Sized>(r: &R, project: ProjectId) -> AppResult<String> {
    Ok(r.project(project)?
        .map_or_else(String::new, |p| p.key.to_string()))
}

/// The conversation from the person's latest message on, oldest first, when the rule looks at
/// it for this call; empty otherwise.
fn turn_for<R: Reader + ?Sized>(
    r: &R,
    caller: &Caller,
    action: &Action,
    project: ProjectId,
) -> AppResult<Vec<Message>> {
    if !access::needs_turn(caller, action) {
        return Ok(Vec::new());
    }
    let mut recent = r.conversation_page(project, None, 200)?;
    let from = recent
        .iter()
        .rposition(|m| m.author == Author::Person)
        .unwrap_or(recent.len());
    Ok(recent.split_off(from))
}

/// The version of `c`'s release once it is cut or released.
fn cut_version<R: Reader + ?Sized>(r: &R, c: &Chain) -> AppResult<Option<String>> {
    Ok(r.chain_view(c.id)?.and_then(|v| {
        v.finish_release()
            .filter(|rel| rel.state != ReleaseState::Planned)
            .map(|rel| rel.version.clone())
    }))
}

/// Whether `caller` may do `action` on project `project` as a whole.
pub fn on_project<R: Reader + ?Sized>(
    r: &R,
    caller: &Caller,
    action: &Action,
    project: ProjectId,
) -> AppResult<()> {
    let key = key_of(r, project)?;
    let target = Target {
        project: Some((project, &key)),
        ..Target::default()
    };
    Ok(access::may(caller, action, &target)?)
}

/// Whether `caller` may do `action` on epic `c`.
pub fn on_epic<R: Reader + ?Sized>(
    r: &R,
    caller: &Caller,
    action: &Action,
    c: &Chain,
) -> AppResult<()> {
    let key = key_of(r, c.project_id)?;
    let cut = match action {
        Action::EditEpic { .. } => cut_version(r, c)?,
        _ => None,
    };
    let turn = turn_for(r, caller, action, c.project_id)?;
    let target = Target {
        project: Some((c.project_id, &key)),
        epic: Some(c),
        cut: cut.as_deref(),
        story: None,
        turn: &turn,
    };
    Ok(access::may(caller, action, &target)?)
}

/// Whether `caller` may do `action` on story `t`, in its epic.
pub fn on_story<R: Reader + ?Sized>(
    r: &R,
    caller: &Caller,
    action: &Action,
    t: &Ticket,
) -> AppResult<()> {
    let c = r
        .chain(t.chain_id)?
        .ok_or_else(|| AppError::not_found(format!("epic of {}", t.key)))?;
    let key = key_of(r, c.project_id)?;
    let turn = turn_for(r, caller, action, c.project_id)?;
    let target = Target {
        project: Some((c.project_id, &key)),
        epic: Some(&c),
        cut: None,
        story: Some(t),
        turn: &turn,
    };
    Ok(access::may(caller, action, &target)?)
}

/// Tells what the assistant did in its project's conversation (`added story DEMO-3`), so the
/// person reads it in the chat and the next turn's rule sees it; what a person does is not told.
pub fn tell<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    caller: &Caller,
    text: String,
    about: Option<About>,
) -> AppResult<()> {
    match caller.planner_project() {
        Some(project) => crate::conversation::line(uow, project, text, about),
        None => Ok(()),
    }
}

/// Whether `caller` may do `action` on story `t`, looking at the story alone: what a story's run
/// does on its own story (it reads nothing else).
pub fn on_run_story(caller: &Caller, action: &Action, t: &Ticket) -> AppResult<()> {
    let target = Target {
        story: Some(t),
        ..Target::default()
    };
    Ok(access::may(caller, action, &target)?)
}
