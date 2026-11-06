//! The inbox: everything that waits on a person, computed from the project's cards, never
//! stored. Each kind of item is one rule here; the clients only show what this says.
//!
//! | Kind | A card (or thread) is listed while |
//! |---|---|
//! | `proposal` | a wildcard card is Proposed |
//! | `question` | the agent is Waiting with a question or a permission prompt |
//! | `blocked` | the card is Blocked (the item carries the reason) |
//! | `review` | the card is in Review and it is the person's turn (`review.turn`; while the reviewer agent has it, or the merge is running, the card is not listed; a failed merge is a `blocked` item) |
//! | `thread` | a thread is open and its newest version came after the person's last request |
//! | `leftovers` | a card was done out of order and an earlier card still has open subtasks that nobody dismissed |
//!
//! Items come newest first. The person's actions on them are the ordinary ones (approve,
//! reject, unblock, a comment, approve a thread) plus the three leftover actions in
//! [`leftovers`].

pub mod leftovers;

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{
    ChainId, ReviewTurn, StatusReason, Subtask, Thread, ThreadMessage, ThreadVersion, Ticket,
    TicketKey, TicketStatus, Timestamp, WaitingReason, machine::TicketAction, thread,
};

/// What kind of thing waits on the person; the order is the order clients group them in.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InboxKind {
    /// A wildcard card waits for approval.
    Proposal,
    /// The agent asked something and waits for the answer.
    Question,
    /// A card is blocked.
    Blocked,
    /// A card waits for review.
    Review,
    /// A thread has a version the person has not answered.
    Thread,
    /// Earlier cards have open subtasks behind a card done out of order.
    Leftovers,
}

/// One entry of the inbox.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InboxItem {
    pub kind: InboxKind,
    /// The card to open and act on; for `leftovers` the earlier card that holds the subtasks.
    pub ticket: TicketKey,
    pub chain: ChainId,
    /// The card's title.
    pub title: String,
    /// Since when it waits; items are sorted by this, newest first.
    pub since: Timestamp,
    /// The question, the blocking reason, or the thread's title; empty otherwise.
    pub text: String,
    /// The thread's number on the card (`thread` items).
    pub thread: Option<u32>,
    /// The open subtasks, verbatim (`leftovers` items).
    pub subtasks: Vec<String>,
    /// The cards done out of order that these subtasks are behind (`leftovers` items).
    pub done_cards: Vec<TicketKey>,
    /// The card's pull request on GitHub, for a blocked card that has one (failed checks, a
    /// closed pull request, a conflict); absent otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    /// What a person may do to the card now, as the state machine allows it; the window offers
    /// no other card action.
    pub allowed_actions: Vec<TicketAction>,
}

/// A thread with what the rule needs to know about it.
#[derive(Debug, Clone)]
pub struct ThreadFacts {
    pub ticket: TicketKey,
    pub thread: Thread,
    pub versions: Vec<ThreadVersion>,
    pub messages: Vec<ThreadMessage>,
}

/// What the inbox is computed from: the project's cards and the facts of its threads and
/// subtasks.
#[derive(Debug, Clone, Default)]
pub struct InboxData {
    pub tickets: Vec<Ticket>,
    /// Subtasks of the cards that were left behind (`done_out_of_order.behind`).
    pub subtasks: Vec<Subtask>,
    pub threads: Vec<ThreadFacts>,
}

/// The inbox of a project, newest first (ties by key).
pub fn compute(data: &InboxData) -> Vec<InboxItem> {
    let mut items: Vec<InboxItem> = data.tickets.iter().filter_map(card_item).collect();
    items.extend(
        data.threads
            .iter()
            .filter_map(|f| thread_item(f, &data.tickets)),
    );
    items.extend(leftover_items(data));
    items.sort_by(|a, b| b.since.cmp(&a.since).then_with(|| a.ticket.cmp(&b.ticket)));
    items
}

fn item(kind: InboxKind, t: &Ticket, since: Timestamp, text: String) -> InboxItem {
    InboxItem {
        kind,
        ticket: t.key.clone(),
        chain: t.chain_id,
        title: t.title.clone(),
        since,
        text,
        thread: None,
        subtasks: Vec::new(),
        done_cards: Vec::new(),
        link: t
            .pr
            .as_ref()
            .filter(|_| t.status == TicketStatus::Blocked)
            .map(|p| p.url.clone()),
        allowed_actions: TicketAction::allowed(t),
    }
}

/// The item a card makes by its own status, if any.
fn card_item(t: &Ticket) -> Option<InboxItem> {
    let (kind, text) = match (t.status, &t.status_reason) {
        (TicketStatus::Proposed, _) => (InboxKind::Proposal, String::new()),
        (
            TicketStatus::Waiting,
            Some(StatusReason::Waiting(
                WaitingReason::Question { text } | WaitingReason::Permission { text },
            )),
        ) => (InboxKind::Question, text.clone()),
        (TicketStatus::Blocked, reason) => (
            InboxKind::Blocked,
            reason.as_ref().map(ToString::to_string).unwrap_or_default(),
        ),
        // The reviewer agent still has the card (in agent mode it decides, in both mode the
        // person comes after its verdict), or the accepted work is being merged: nobody's turn.
        (TicketStatus::Review, _)
            if matches!(
                t.review.turn,
                Some(ReviewTurn::Agent { .. } | ReviewTurn::Merging)
            ) =>
        {
            return None;
        }
        (TicketStatus::Review, _) => (InboxKind::Review, String::new()),
        _ => return None,
    };
    Some(item(kind, t, t.status_since, text))
}

fn thread_item(f: &ThreadFacts, tickets: &[Ticket]) -> Option<InboxItem> {
    let since = thread::awaiting_person(&f.thread, &f.versions, &f.messages)?;
    let card = tickets.iter().find(|t| t.id == f.thread.ticket_id)?;
    let mut i = item(InboxKind::Thread, card, since, f.thread.title.clone());
    i.thread = Some(f.thread.number);
    Some(i)
}

/// One item per earlier card that has open subtasks behind a card done out of order.
fn leftover_items(data: &InboxData) -> Vec<InboxItem> {
    let behind: BTreeSet<&TicketKey> = data
        .tickets
        .iter()
        .filter_map(|t| t.done_out_of_order.as_ref())
        .flat_map(|m| &m.behind)
        .collect();
    behind
        .into_iter()
        .filter_map(|key| {
            let card = data.tickets.iter().find(|t| &t.key == key)?;
            let open = leftovers::open_subtasks(&data.subtasks, card.id);
            let marks = leftovers::marks_awaiting(&data.tickets, key);
            let since = marks.iter().map(|m| m.status_since).max()?;
            if open.is_empty() {
                return None;
            }
            let mut i = item(InboxKind::Leftovers, card, since, String::new());
            i.subtasks = open.into_iter().map(|s| s.text).collect();
            i.done_cards = marks.iter().map(|m| m.key.clone()).collect();
            Some(i)
        })
        .collect()
}

#[cfg(test)]
mod tests;
