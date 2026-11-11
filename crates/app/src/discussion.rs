//! A card's discussion with the project's assistant, kept as a thread of kind `discussion`: the
//! person's messages, the assistant's answers (it posts into the thread) and its edit proposals,
//! which land only when a person accepts them. The rules are `toknila_core::discussion`.

use toknila_core::discussion::{self, Decision, EditProposal};
use toknila_core::machine::person_only;
use toknila_core::{
    MessageKind, Thread, ThreadKind, ThreadMessage, ThreadMessageId, Ticket, TicketPatch,
};

use crate::{AppError, AppResult, Reader, UnitOfWork, threads, tickets};

/// The card's discussion thread, if it has one.
pub fn find<R: Reader + ?Sized>(r: &R, t: &Ticket) -> AppResult<Option<Thread>> {
    Ok(r.threads(t.id)?
        .into_iter()
        .find(|th| th.kind == ThreadKind::Discussion))
}

/// A person says something on the card: the message goes in the card's discussion, which is
/// opened by the first one. The text for the planner is the host's to send.
pub fn say<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    t: &Ticket,
    text: &str,
) -> AppResult<(Thread, ThreadMessage)> {
    person_only(uow.actor(), "talk to the assistant")?;
    let th = match find(&*uow, t)? {
        Some(th) => th,
        None => threads::open(uow, t, ThreadKind::Discussion, discussion::TITLE)?,
    };
    let m = threads::post_message(uow, t, &th, text)?;
    Ok((th, m))
}

/// The assistant proposes a change of the card; nothing changes until a person accepts it.
pub fn propose<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    t: &Ticket,
    th: &Thread,
    proposal: &EditProposal,
) -> AppResult<ThreadMessage> {
    let text = serde_json::to_string(proposal)
        .map_err(|e| AppError::internal(format!("an edit proposal: {e}")))?;
    threads::message(uow, t, th, MessageKind::EditProposal, &text)
}

/// A person accepts or rejects the pending proposal in `message` (its id). Accepting changes the
/// card, unless the card is no longer what the proposal was made against (a conflict).
pub fn decide<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    t: &Ticket,
    th: &Thread,
    message: &str,
    accept: bool,
) -> AppResult<ThreadMessage> {
    person_only(uow.actor(), "decide on a proposed edit")?;
    let id: ThreadMessageId = message
        .parse()
        .map_err(|_| AppError::not_found(format!("proposal {message}")))?;
    let all = uow.thread_messages(th.id)?;
    let found = all
        .iter()
        .find(|m| m.id == id && m.kind == MessageKind::EditProposal)
        .ok_or_else(|| AppError::not_found(format!("proposal {message} of {}", t.key)))?;
    let proposal = discussion::proposal_of(found)
        .ok_or_else(|| AppError::internal("the proposal cannot be read"))?;
    if discussion::decision(&all, found) != Decision::Pending {
        return Err(AppError::conflict("this proposal is already decided"));
    }
    if !accept {
        return threads::message(uow, t, th, MessageKind::EditRejected, message);
    }
    // The card as it is now, not as the caller saw it.
    let now = uow
        .ticket(t.id)?
        .ok_or_else(|| AppError::not_found(format!("ticket {}", t.key)))?;
    if !proposal.applies_to(&now.prompt, &now.acceptance) {
        return Err(AppError::conflict(format!(
            "{} changed since this was proposed; ask the assistant again",
            t.key
        )));
    }
    let patch = TicketPatch {
        prompt: proposal.prompt.map(|c| c.to),
        acceptance: proposal.acceptance.map(|c| c.to),
        ..TicketPatch::default()
    };
    tickets::edit(uow, &now, patch)?;
    let m = threads::message(uow, t, th, MessageKind::EditAccepted, message)?;
    threads::tell_worker(
        uow,
        t,
        "The person accepted a change of this card's prompt or acceptance; read the card again."
            .to_owned(),
        None,
    )?;
    Ok(m)
}

/// What the card's agent reads of the discussion.
pub fn transcript<R: Reader + ?Sized>(
    r: &R,
    ticket: toknila_core::TicketId,
) -> AppResult<Vec<String>> {
    let mut lines = Vec::new();
    for th in r.threads(ticket)? {
        if th.kind == ThreadKind::Discussion {
            lines.extend(discussion::transcript(&r.thread_messages(th.id)?));
        }
    }
    Ok(lines)
}
