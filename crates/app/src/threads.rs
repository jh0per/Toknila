//! Threads on a card: settle the design with the owner before implementation. A person or the
//! ticket's agent opens a thread and posts messages and artifact versions; only a person approves
//! or requests changes (core refuses anyone else). The hand-in gate is core's (`TicketEvent::Finished`).

use serde::{Deserialize, Serialize};
use toknila_core::events::DomainEvent;
use toknila_core::thread::{self, ThreadChange, ThreadEvent};
use toknila_core::{
    ArtifactFormat, CommentBody, CommentKind, MessageKind, StatusReason, Thread, ThreadId,
    ThreadKind, ThreadMessage, ThreadMessageId, ThreadVersion, ThreadVersionId, Ticket,
    TicketStatus, WaitingReason,
};

use crate::{AppError, AppResult, UnitOfWork};

/// `POST /v1/tickets/{ticket}/threads`.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NewThread {
    pub kind: ThreadKind,
    pub title: String,
}

/// Body of `.../messages` and `.../changes`.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NewMessage {
    pub text: String,
}

/// `POST /v1/tickets/{ticket}/threads/{thread}/versions`.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NewVersion {
    pub format: ArtifactFormat,
    pub body: String,
    /// A message posted with the version, in the same transaction.
    #[serde(default)]
    pub message: Option<String>,
}

/// Opens a thread on a ticket, by the command's actor.
pub fn open<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    t: &Ticket,
    kind: ThreadKind,
    title: &str,
) -> AppResult<Thread> {
    let all = uow.threads(t.id)?;
    if uow.actor() != toknila_core::Actor::Human
        && let Some(had) = thread::existing_of_kind(&all, kind)
    {
        return Err(thread::ThreadError::DuplicateKind {
            kind,
            number: had.number,
        }
        .into());
    }
    let number = u32::try_from(all.len())
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
    let head = format!("Thread {} \"{}\": changes requested.", th.number, th.title);
    tell_worker(uow, t, head, Some(m.text.clone()))?;
    Ok(m)
}

/// Posts the next artifact version. An approved thread reopens.
pub fn post_version<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    t: &Ticket,
    th: &Thread,
    input: NewVersion,
) -> AppResult<ThreadVersion> {
    if th.kind == ThreadKind::Discussion {
        return Err(AppError::bad_request(
            "a discussion has no design versions; open a design thread",
        ));
    }
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
    // Checked before anything is written, so a blank message refuses the whole post.
    let note = input
        .message
        .map(|text| {
            thread::message(
                th,
                ThreadMessageId::new(),
                by,
                MessageKind::Message,
                &text,
                now,
            )
        })
        .transpose()?;
    uow.insert_thread_version(&v)?;
    uow.emit(&DomainEvent::ThreadVersionPosted {
        ticket: t.key.clone(),
        thread: th.id,
        number,
        format: v.format,
        author: by,
        bytes: v.body.len(),
    })?;
    if let Some(m) = note {
        uow.insert_thread_message(&m)?;
        uow.emit(&DomainEvent::ThreadMessagePosted {
            ticket: t.key.clone(),
            message: m,
        })?;
    }
    save(uow, t, &next, change)?;
    Ok(v)
}

/// The agent withdraws its own thread while it has no version and no person message.
pub fn withdraw<U: UnitOfWork + ?Sized>(uow: &mut U, t: &Ticket, th: &Thread) -> AppResult<Thread> {
    let versions = uow.thread_versions(th.id)?.len();
    let person_messages = uow
        .thread_messages(th.id)?
        .iter()
        .filter(|m| m.author == toknila_core::Actor::Human)
        .count();
    let event = ThreadEvent::Withdraw {
        by: uow.actor(),
        versions,
        person_messages,
    };
    let (next, change) = thread::apply(th, event, uow.now())?;
    save(uow, t, &next, change)?;
    Ok(next)
}

/// A person approves a thread; the latest version, if any, is pinned.
pub fn approve<U: UnitOfWork + ?Sized>(uow: &mut U, t: &Ticket, th: &Thread) -> AppResult<Thread> {
    let event = ThreadEvent::Approve {
        by: uow.actor(),
        latest_version: latest_number(uow, th)?,
    };
    let (next, change) = thread::apply(th, event, uow.now())?;
    save(uow, t, &next, change)?;
    let said = last_person_message(uow, th)?;
    let version = next
        .approved_version
        .map(|v| format!(" version {v}"))
        .unwrap_or_default();
    let head = format!(
        "Thread {} \"{}\"{version} is approved.",
        th.number, th.title
    );
    tell_worker(uow, t, head, said)?;
    Ok(next)
}

/// The person's last message in the thread, if any.
fn last_person_message<R: crate::Reader + ?Sized>(r: &R, th: &Thread) -> AppResult<Option<String>> {
    Ok(r.thread_messages(th.id)?
        .into_iter()
        .rfind(|m| m.author == toknila_core::Actor::Human)
        .map(|m| m.text))
}

/// A decision on a thread reaches the card's worker as an instruction comment, through the same
/// path as `tok comment`: typed into the live agent or kept for its next hook. A worker that
/// waits on its own `ask_human` question is answered by it; a permission prompt is not a
/// question about the thread and stays open, and a card in any other status is left alone.
pub(crate) fn tell_worker<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    t: &Ticket,
    head: String,
    said: Option<String>,
) -> AppResult<()> {
    let Some(now) = uow.ticket(t.id)? else {
        return Ok(());
    };
    let wanted = matches!(
        (now.status, &now.status_reason),
        (TicketStatus::Running, _)
            | (
                TicketStatus::Waiting,
                Some(StatusReason::Waiting(WaitingReason::Question { .. }))
            )
    );
    if !wanted {
        return Ok(());
    }
    let said = said.map(|s| s.split_whitespace().collect::<Vec<_>>().join(" "));
    let gist = match said.as_deref().map(|s| s.trim_end_matches('.')) {
        Some(s) if !s.is_empty() => format!("{head} {s}. Continue."),
        _ => format!("{head} Continue."),
    };
    let body = CommentBody {
        gist,
        ..Default::default()
    };
    crate::tickets::comment(uow, t, CommentKind::Instruction, body)
}

/// The ticket's open threads, for the hand-in event; the state machine refuses the hand-in
/// while there are any.
pub fn open_threads<R: crate::Reader + ?Sized>(
    r: &R,
    t: &Ticket,
) -> AppResult<Vec<thread::OpenThread>> {
    let all = r.threads(t.id)?;
    let mut with_version = std::collections::HashSet::new();
    for th in &all {
        if !r.thread_versions(th.id)?.is_empty() {
            with_version.insert(th.id);
        }
    }
    Ok(thread::open_threads(&all, |th| {
        with_version.contains(&th.id)
    }))
}

/// The design gate of the hand-in: a card that must show a design first is refused until one
/// of its threads is approved.
pub fn require_design<R: crate::Reader + ?Sized>(r: &R, t: &Ticket) -> AppResult<()> {
    Ok(thread::require_design(t, &r.threads(t.id)?)?)
}

pub(crate) fn message<U: UnitOfWork + ?Sized>(
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
        ThreadChange::Withdrawn => DomainEvent::ThreadWithdrawn { ticket, thread },
    })
}
