//! A story as any caller adds, edits, shows and talks about it.

use toknila_core::access::{proposes_edits, story_model};
use toknila_core::conversation::{self, About};
use toknila_core::discussion::EditProposal;
use toknila_core::events::DomainEvent;
use toknila_core::{Thread, ThreadKind, Ticket, TicketId, TicketPatch};

use crate::access::{self, Action, Caller};
use crate::tickets::{self, NewTicket};
use crate::{AppError, AppResult, Reader, UnitOfWork, discussion, notes, resolve, threads};

fn about(t: &Ticket) -> Option<About> {
    Some(About::Card { key: t.key.clone() })
}

/// Adds a story to an epic. A planner adds only its own fields to an epic that waits for the
/// owner to start its stories, and names a model by alias or full name.
pub fn add_story<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    caller: &Caller,
    chain: &str,
    mut input: NewTicket,
) -> AppResult<TicketId> {
    let c = access::chain(&*uow, caller, None, chain)?;
    let add = Action::AddStory {
        owner_fields: input.sets_owner_fields(),
    };
    access::on_epic(&*uow, caller, &add, &c)?;
    input.model = match input.model.as_deref() {
        Some(m) => story_model(caller, m).map_err(AppError::bad_request)?,
        None => None,
    };
    let id = tickets::create(uow, &c.id.to_string(), input)?;
    if let Some(t) = uow.ticket(id)? {
        access::tell(uow, caller, conversation::added_card(&t.key), about(&t))?;
    }
    Ok(id)
}

/// Edits story `t`: the patch's fields, then its checklist items renamed and appended. A planner
/// edits one that has not started, only its words, checklist, designs and model (an alias or a
/// full name), and its new prompt or acceptance on a story the person talks over with it is a
/// proposal that lands when the person accepts it. Designs are named by id here.
pub fn edit_story<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    caller: &Caller,
    t: &Ticket,
    mut patch: TicketPatch,
) -> AppResult<TicketId> {
    let add_subtasks = std::mem::take(&mut patch.add_subtasks);
    let edit_subtasks = std::mem::take(&mut patch.edit_subtasks);
    let action = Action::EditStory {
        owner_fields: patch.sets_owner_fields(),
    };
    access::on_story(&*uow, caller, &action, t)?;
    if let Some(m) = patch.model.as_deref() {
        story_model(caller, m).map_err(AppError::bad_request)?;
    }
    let mut proposed = false;
    if proposes_edits(caller)
        && (patch.prompt.is_some() || patch.acceptance.is_some())
        && let Some(th) = discussion::find(&*uow, t)?
    {
        let (prompt, acceptance) = (patch.prompt.take(), patch.acceptance.take());
        let proposal = EditProposal::of(&t.prompt, &t.acceptance, prompt, acceptance);
        if !proposal.is_empty() {
            discussion::propose(uow, t, &th, &proposal)?;
            proposed = true;
        }
    }
    let changed = patch != TicketPatch::default();
    let nothing_else = !proposed && add_subtasks.is_empty() && edit_subtasks.is_empty();
    // A patch alone is an edit even when empty, as it always was: its refusals still apply.
    if changed || nothing_else {
        tickets::edit(uow, t, patch)?;
    }
    for item in &edit_subtasks {
        notes::update_subtask(uow, t, item.position, None, Some(item.text.clone()))?;
    }
    for text in &add_subtasks {
        notes::add_subtask(uow, t, text)?;
    }
    // A change only proposed is not yet an edit: the story has it when the person accepts.
    let text = if proposed && !changed && edit_subtasks.is_empty() && add_subtasks.is_empty() {
        conversation::proposed_edit(&t.key)
    } else {
        conversation::edited_card(&t.key)
    };
    access::tell(uow, caller, text, about(t))?;
    Ok(t.id)
}

/// Asks the window to show a story: one `window.show_card` event, no change to the plan.
/// Whether a window follows is for the window to decide.
pub fn show_story<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    caller: &Caller,
    story: &str,
) -> AppResult<Ticket> {
    let t = resolve::ticket(&*uow, story)?;
    access::on_story(&*uow, caller, &Action::ShowStory, &t)?;
    let chain = uow
        .chain(t.chain_id)?
        .ok_or_else(|| AppError::not_found(format!("epic of {}", t.key)))?;
    let p = uow
        .project(chain.project_id)?
        .ok_or_else(|| AppError::not_found(format!("project {}", chain.project_id)))?;
    uow.emit(&DomainEvent::WindowShowCard {
        project: p.id,
        project_key: p.key,
        chain: chain.name,
        ticket: t.key.clone(),
    })?;
    Ok(t)
}

/// Opens a thread on a story.
pub fn open_thread<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    caller: &Caller,
    story: &str,
    kind: ThreadKind,
    title: &str,
) -> AppResult<(Ticket, Thread)> {
    let t = resolve::ticket(&*uow, story)?;
    access::on_story(&*uow, caller, &Action::Thread, &t)?;
    let th = threads::open(uow, &t, kind, title)?;
    let text = conversation::opened_thread(th.number, &t.key);
    access::tell(uow, caller, text, about(&t))?;
    Ok((t, th))
}

/// A thread of a story, by number or id, that `caller` may write in.
pub fn thread<R: Reader + ?Sized>(
    r: &R,
    caller: &Caller,
    story: &str,
    reference: &str,
) -> AppResult<(Ticket, Thread)> {
    let t = resolve::ticket(r, story)?;
    access::on_story(r, caller, &Action::Thread, &t)?;
    let th = threads::find(r, &t, reference)?;
    Ok((t, th))
}
