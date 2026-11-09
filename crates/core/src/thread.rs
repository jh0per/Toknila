//! Threads on a card: the rules. A thread is opened by a person or the ticket's agent, holds
//! messages and numbered immutable artifact versions, and ends in a person's approval. While a
//! ticket has an open thread, the agent's hand-in is refused (see `TicketEvent::Finished`).
//!
//! | From | To | Event |
//! |---|---|---|
//! | Open | Approved | `Approve` (a person only; pins the latest version, if any) |
//! | Approved | Open | `NewVersion` (by anyone), `ChangesRequested` (a person only) |
//! | Open | Open | `NewVersion`, `ChangesRequested` |
//!
//! Everything here is pure: ids and times come in as arguments.

use std::fmt;

use crate::machine::TransitionError;
use crate::{
    Actor, ArtifactFormat, MessageKind, Phase, Thread, ThreadId, ThreadKind, ThreadMessage,
    ThreadMessageId, ThreadState, ThreadVersion, ThreadVersionId, Ticket, TicketId, TicketKey,
    Timestamp,
};

/// Largest artifact body, in bytes (512 KiB).
pub const MAX_ARTIFACT_BYTES: usize = 512 * 1024;

/// A thread that still blocks the hand-in, as the error names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenThread {
    pub number: u32,
    pub kind: ThreadKind,
    pub title: String,
}

impl fmt::Display for OpenThread {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{} \"{}\" ({:?})", self.number, self.title, self.kind)
    }
}

/// Why a thread rule refused. Nothing changed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ThreadError {
    #[error("thread title is empty")]
    EmptyTitle,
    #[error("{0} needs a text")]
    EmptyText(&'static str),
    #[error("artifact body is empty")]
    EmptyArtifact,
    #[error("artifact is {bytes} bytes, the limit is {max}")]
    ArtifactTooLarge { bytes: usize, max: usize },
    #[error("only a person can {action}; this came from {actor:?}")]
    PersonOnly { action: &'static str, actor: Actor },
    #[error("thread is already approved")]
    AlreadyApproved,
}

pub(crate) fn list(threads: &[OpenThread]) -> String {
    let parts: Vec<String> = threads.iter().map(ToString::to_string).collect();
    format!("{} open: {}", threads.len(), parts.join(", "))
}

/// Something that happens to a thread's state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadEvent {
    /// A person approves; `latest_version` is the newest version number, which gets pinned.
    Approve {
        by: Actor,
        latest_version: Option<u32>,
    },
    /// A new artifact version was posted.
    NewVersion,
    /// A person asks for changes.
    ChangesRequested { by: Actor },
}

/// What a transition did to the thread's state, for the journal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadChange {
    Approved,
    Reopened,
}

/// A new, open thread.
pub fn open(
    id: ThreadId,
    ticket: TicketId,
    number: u32,
    kind: ThreadKind,
    title: &str,
    by: Actor,
    now: Timestamp,
) -> Result<Thread, ThreadError> {
    let title = title.trim();
    if title.is_empty() {
        return Err(ThreadError::EmptyTitle);
    }
    Ok(Thread {
        id,
        ticket_id: ticket,
        number,
        kind,
        title: title.to_owned(),
        state: ThreadState::Open,
        opened_by: by,
        created_at: now,
        approved_version: None,
        approved_at: None,
    })
}

/// A message in `thread`; it does not change the thread's state.
pub fn message(
    thread: &Thread,
    id: ThreadMessageId,
    by: Actor,
    kind: MessageKind,
    text: &str,
    now: Timestamp,
) -> Result<ThreadMessage, ThreadError> {
    let text = text.trim();
    if text.is_empty() {
        return Err(ThreadError::EmptyText(match kind {
            MessageKind::Message => "message",
            MessageKind::ChangesRequested => "request for changes",
        }));
    }
    Ok(ThreadMessage {
        id,
        thread_id: thread.id,
        author: by,
        kind,
        text: text.to_owned(),
        created_at: now,
    })
}

/// Version `number` of `thread`'s artifact. Versions are immutable once stored.
pub fn version(
    thread: &Thread,
    id: ThreadVersionId,
    number: u32,
    by: Actor,
    format: ArtifactFormat,
    body: String,
    now: Timestamp,
) -> Result<ThreadVersion, ThreadError> {
    if body.trim().is_empty() {
        return Err(ThreadError::EmptyArtifact);
    }
    if body.len() > MAX_ARTIFACT_BYTES {
        return Err(ThreadError::ArtifactTooLarge {
            bytes: body.len(),
            max: MAX_ARTIFACT_BYTES,
        });
    }
    Ok(ThreadVersion {
        id,
        thread_id: thread.id,
        number,
        author: by,
        format,
        body,
        created_at: now,
    })
}

/// Applies `event` to `thread`. Pure: same input, same output.
pub fn apply(
    thread: &Thread,
    event: ThreadEvent,
    now: Timestamp,
) -> Result<(Thread, Option<ThreadChange>), ThreadError> {
    let mut next = thread.clone();
    let reopen = |next: &mut Thread| {
        let was = next.state;
        next.state = ThreadState::Open;
        next.approved_version = None;
        next.approved_at = None;
        (was == ThreadState::Approved).then_some(ThreadChange::Reopened)
    };
    let change = match event {
        ThreadEvent::Approve { by, latest_version } => {
            person_only(by, "approve a thread")?;
            if thread.state == ThreadState::Approved {
                return Err(ThreadError::AlreadyApproved);
            }
            next.state = ThreadState::Approved;
            next.approved_version = latest_version;
            next.approved_at = Some(now);
            Some(ThreadChange::Approved)
        }
        ThreadEvent::NewVersion => reopen(&mut next),
        ThreadEvent::ChangesRequested { by } => {
            person_only(by, "request changes")?;
            reopen(&mut next)
        }
    };
    Ok((next, change))
}

fn person_only(by: Actor, action: &'static str) -> Result<(), ThreadError> {
    if by == Actor::Human {
        Ok(())
    } else {
        Err(ThreadError::PersonOnly { action, actor: by })
    }
}

/// The ticket's phase: design while any thread is open, implement otherwise.
pub fn phase(threads: &[Thread]) -> Phase {
    if threads.iter().any(|t| t.state == ThreadState::Open) {
        Phase::Design
    } else {
        Phase::Implement
    }
}

/// Since when an open thread waits for the person: its newest version, as long as no request
/// for changes came after it. `None` when the thread is approved, has no version yet, or waits
/// for the agent to answer a request.
pub fn awaiting_person(
    thread: &Thread,
    versions: &[ThreadVersion],
    messages: &[ThreadMessage],
) -> Option<Timestamp> {
    if thread.state != ThreadState::Open {
        return None;
    }
    let newest = versions.iter().map(|v| v.created_at).max()?;
    let asked = messages
        .iter()
        .filter(|m| m.kind == MessageKind::ChangesRequested)
        .map(|m| m.created_at)
        .max();
    asked.is_none_or(|at| newest > at).then_some(newest)
}

/// Whether an open thread has a person's request for changes that no newer version answered.
/// An approved thread never does; a version posted at the instant of the request does not answer it.
pub fn changes_requested(
    thread: &Thread,
    versions: &[ThreadVersion],
    messages: &[ThreadMessage],
) -> bool {
    if thread.state != ThreadState::Open {
        return false;
    }
    let asked = messages
        .iter()
        .filter(|m| m.kind == MessageKind::ChangesRequested)
        .map(|m| m.created_at)
        .max();
    asked.is_some_and(|at| !versions.iter().any(|v| v.created_at > at))
}

/// The hand-in gate: refused, naming them, while any thread is open.
pub fn require_none_open(ticket: &TicketKey, open: &[OpenThread]) -> Result<(), TransitionError> {
    if open.is_empty() {
        return Ok(());
    }
    Err(TransitionError::OpenThreads(ticket.clone(), open.to_vec()))
}

/// The instruction a person's "ask for a design" sends to a card that already runs.
pub const ASK_DESIGN_TEXT: &str = "Before you go further, open a design thread for this card and wait for approval. Use `open_thread` (kind design), post a first version the person can judge with `thread_version`, and write code only after the person approved a version.";

/// The design gate of the hand-in: a card that must show a design first is refused until one of
/// its threads is approved with a version pinned.
pub fn require_design(ticket: &Ticket, threads: &[Thread]) -> Result<(), TransitionError> {
    let approved = threads
        .iter()
        .any(|t| t.state == ThreadState::Approved && t.approved_version.is_some());
    if !ticket.design_first || approved {
        return Ok(());
    }
    Err(TransitionError::DesignNotApproved(ticket.key.clone()))
}

/// The ticket's threads that are still open. The agent's hand-in carries this list and the
/// state machine refuses it unless it is empty.
pub fn open_threads(threads: &[Thread]) -> Vec<OpenThread> {
    threads
        .iter()
        .filter(|t| t.state == ThreadState::Open)
        .map(|t| OpenThread {
            number: t.number,
            kind: t.kind,
            title: t.title.clone(),
        })
        .collect()
}

#[cfg(test)]
mod tests;
