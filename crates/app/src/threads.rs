//! Threads on a card: settle the design with the owner before implementation. A person or the
//! ticket's agent opens a thread and posts messages and artifact versions; only a person approves
//! or requests changes (core refuses anyone else). The hand-in gate is [`check_hand_in`].

use serde::{Deserialize, Serialize};
use toknila_core::events::DomainEvent;
use toknila_core::thread::{self, ThreadChange, ThreadEvent};
use toknila_core::{
    ArtifactFormat, MessageKind, Thread, ThreadId, ThreadKind, ThreadMessage, ThreadMessageId,
    ThreadVersion, ThreadVersionId, Ticket,
};

use crate::{AppError, AppResult, UnitOfWork};

/// `POST /v1/tickets/{ticket}/threads`.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NewThread {
    pub kind: ThreadKind,
    pub title: String,
}

/// Body of `.../messages` and `.../changes`.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NewMessage {
    pub text: String,
}

/// `POST /v1/tickets/{ticket}/threads/{thread}/versions`.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NewVersion {
    pub format: ArtifactFormat,
    pub body: String,
}

/// Opens a thread on a ticket, by the command's actor.
pub fn open<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    t: &Ticket,
    kind: ThreadKind,
    title: &str,
) -> AppResult<Thread> {
    let number = u32::try_from(uow.threads(t.id)?.len())
        .unwrap_or(u32::MAX)
        .saturating_add(1);
    let th = thread::open(
        ThreadId::new(),
        t.id,
        number,
        kind,
        title,
        uow.actor(),
        uow.now(),
    )?;
    uow.upsert_thread(&th)?;
    uow.emit(&DomainEvent::ThreadOpened {
        ticket: t.key.clone(),
        thread: th.clone(),
    })?;
    Ok(th)
}

/// A ticket's thread by its number on the ticket (`2`) or by id.
pub fn find<R: crate::Reader + ?Sized>(r: &R, t: &Ticket, reference: &str) -> AppResult<Thread> {
    let reference = reference.trim().trim_start_matches('#');
    let threads = r.threads(t.id)?;
    let found = match reference.parse::<u32>() {
        Ok(number) => threads.into_iter().find(|th| th.number == number),
        Err(_) => reference
            .parse::<ThreadId>()
            .ok()
            .and_then(|id| threads.into_iter().find(|th| th.id == id)),
    };
    found.ok_or_else(|| AppError::not_found(format!("thread {reference} of {}", t.key)))
}

/// Posts a message in a thread; the thread's state does not change.
pub fn post_message<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    t: &Ticket,
    th: &Thread,
    text: &str,
) -> AppResult<ThreadMessage> {
    message(uow, t, th, MessageKind::Message, text)
}

/// A person asks for changes: a message that keeps the thread open, or puts it back to open.
pub fn request_changes<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    t: &Ticket,
    th: &Thread,
    text: &str,
) -> AppResult<ThreadMessage> {
    let event = ThreadEvent::ChangesRequested { by: uow.actor() };
    let (next, change) = thread::apply(th, event, uow.now())?;
    let m = message(uow, t, th, MessageKind::ChangesRequested, text)?;
    save(uow, t, &next, change)?;
    Ok(m)
}

/// Posts the next artifact version. An approved thread reopens.
pub fn post_version<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    t: &Ticket,
    th: &Thread,
    input: NewVersion,
) -> AppResult<ThreadVersion> {
    let number = latest_number(uow, th)?.unwrap_or(0).saturating_add(1);
    let (by, now) = (uow.actor(), uow.now());
    let v = thread::version(
        th,
        ThreadVersionId::new(),
        number,
        by,
        input.format,
        input.body,
        now,
    )?;
    let (next, change) = thread::apply(th, ThreadEvent::NewVersion, now)?;
    uow.insert_thread_version(&v)?;
    uow.emit(&DomainEvent::ThreadVersionPosted {
        ticket: t.key.clone(),
        thread: th.id,
        number,
        format: v.format,
        author: by,
        bytes: v.body.len(),
    })?;
    save(uow, t, &next, change)?;
    Ok(v)
}

/// A person approves a thread; the latest version, if any, is pinned.
pub fn approve<U: UnitOfWork + ?Sized>(uow: &mut U, t: &Ticket, th: &Thread) -> AppResult<Thread> {
    let event = ThreadEvent::Approve {
        by: uow.actor(),
        latest_version: latest_number(uow, th)?,
    };
    let (next, change) = thread::apply(th, event, uow.now())?;
    save(uow, t, &next, change)?;
    Ok(next)
}

/// Refuses the agent's hand-in while any thread of the ticket is open.
pub fn check_hand_in<R: crate::Reader + ?Sized>(r: &R, t: &Ticket) -> AppResult<()> {
    Ok(thread::check_hand_in(&t.key, &r.threads(t.id)?)?)
}

fn message<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    t: &Ticket,
    th: &Thread,
    kind: MessageKind,
    text: &str,
) -> AppResult<ThreadMessage> {
    let m = thread::message(
        th,
        ThreadMessageId::new(),
        uow.actor(),
        kind,
        text,
        uow.now(),
    )?;
    uow.insert_thread_message(&m)?;
    uow.emit(&DomainEvent::ThreadMessagePosted {
        ticket: t.key.clone(),
        message: m.clone(),
    })?;
    Ok(m)
}

fn latest_number<R: crate::Reader + ?Sized>(r: &R, th: &Thread) -> AppResult<Option<u32>> {
    Ok(r.thread_versions(th.id)?.iter().map(|v| v.number).max())
}

/// Stores the thread after a transition and journals the change of state, if any.
fn save<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    t: &Ticket,
    next: &Thread,
    change: Option<ThreadChange>,
) -> AppResult<()> {
    let Some(change) = change else {
        return Ok(());
    };
    uow.upsert_thread(next)?;
    let (ticket, thread) = (t.key.clone(), next.clone());
    uow.emit(&match change {
        ThreadChange::Approved => DomainEvent::ThreadApproved { ticket, thread },
        ThreadChange::Reopened => DomainEvent::ThreadReopened { ticket, thread },
    })
}
