//! The inbox: everything that waits on a person, computed from the project's cards, never
//! stored. Each kind of item is one rule here; the clients only show what this says.
//!
//! | Kind | A card (or thread) is listed while |
//! |---|---|
//! | `proposal` | a wildcard card is Proposed |
//! | `question` | the agent is Waiting with a question or a permission prompt; a reviewer (or any run) shows a permission dialog; or a run showed no sign of life for `startup_secs` after its spawn and waits on its screen (see [`crate::question`]) |
//! | `blocked` | the card is Blocked (the item carries the reason) |
//! | `question` (no checks) | the card's pull request reported no check for the project's `no_checks_timeout_secs`; answers: `merge_now`, `keep_waiting`, `block` |
//! | `review` | the card is in Review and it is the person's turn (`review.turn`; while the reviewer agent has it, or the merge is running, the card is not listed; a failed merge is a `blocked` item) |
//! | `thread` | a thread is open and its newest version came after the person's last request |
//! | `leftovers` | a card was done out of order and an earlier card still has open subtasks that nobody dismissed |
//! | `design_question` | a pin of the assistant on a design is about the card and the person has not answered it |
//! | `pin` | an open pin of the assistant has no card (or its card is not in the project); it is answered in place |
//!
//! Items come newest first. The person's actions on them are the ordinary ones (approve,
//! reject, unblock, a comment, approve a thread) plus the three leftover actions in
//! [`leftovers`].

pub mod hand_in;
pub mod leftovers;
mod pins;
mod screens;

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::design::pin::DesignPin;
use crate::question::{AnswerKind, PendingPermission, QuestionKind, RunFacts};
use crate::{
    AgentRole, BlockedReason, ChainId, Comment, ProjectId, ReviewNote, ReviewTurn, StatusReason,
    Subtask, Thread, ThreadMessage, ThreadVersion, Ticket, TicketKey, TicketStatus, Timestamp,
    WaitingReason, machine::TicketAction, thread,
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
    /// The assistant pinned a question on a design and waits for the answer.
    DesignQuestion,
    /// The assistant pinned a question on a design and no card owns it; answered in place.
    Pin,
}

/// One entry of the inbox.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InboxItem {
    pub kind: InboxKind,
    /// The card to open and act on; for `leftovers` the earlier card that holds the subtasks;
    /// absent for a `pin`, which no card owns.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ticket: Option<TicketKey>,
    /// The card's chain; absent for a `pin`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chain: Option<ChainId>,
    /// The project of the pin to answer (`design_question` and `pin` items).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<ProjectId>,
    /// The card's title; the design's name for a `pin`.
    pub title: String,
    /// Since when it waits; items are sorted by this, newest first.
    pub since: Timestamp,
    /// The question, the blocking reason, or the thread's title; empty otherwise.
    pub text: String,
    /// The whole command of a permission request, when `text` carries only its summary (cut for
    /// lists); absent otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
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
    /// Whose question it is (`question` items): the card's worker or its reviewer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<AgentRole>,
    /// How the question can be answered, as core allows it (`question` items; empty for an
    /// `ask_human` question, which a comment answers).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub answers: Vec<AnswerKind>,
    /// The things the person can do about an `ask_human` question, the recommended one first
    /// (`question` items; empty when the agent offered none).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub choices: Vec<String>,
    /// The last lines of the agent's screen, ANSI removed (`question` items of an agent that
    /// waits on its screen).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lines: Vec<String>,
    /// The pin to answer (`design_question` items).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pin: Option<String>,
    /// The rule "Allow always" would add (`Bash(cargo test *)`), for a worker's question about a
    /// command that can become one; absent otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_always: Option<String>,
    /// Why "Allow always" is not offered for a worker's question about a command (a command built
    /// with `$` cannot become a rule); absent otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_always_none: Option<String>,
    /// The hand-in of the card (`review` items whose agent handed in with the template); absent
    /// otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hand_in: Option<hand_in::InboxHandIn>,
    /// The name of the design a pin is about (`design_question` items).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub design: Option<String>,
}

/// Says on a worker's permission question what "Allow always" would add, or why nothing.
fn set_allow_always(i: &mut InboxItem) {
    match crate::model::command_policy::allow_always_for(i.detail.as_deref(), &i.text) {
        Some(Ok(rule)) => i.allow_always = Some(rule),
        Some(Err(why)) => i.allow_always_none = Some(why.to_owned()),
        None => {}
    }
}

/// The text of the `blocked` item for a card whose runner was lost twice.
pub const RUNNER_LOST_TEXT: &str = "The app restarted twice while the agent worked. Unblock starts it once more; it gets your answer.";

/// The text of the `review` item for a card whose reviewer gave no verdict.
pub const NO_VERDICT_TEXT: &str = "The reviewer gave no verdict";

/// The text of the `question` item for a pull request that reported no check in time; the
/// pull request's URL follows. The answers are the card's actions `merge_now`, `keep_waiting`
/// and `block`.
pub const NO_CHECKS_TEXT: &str =
    "The pull request has no checks. Merge now, keep waiting, or block";

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
#[derive(Debug, Clone)]
pub struct InboxData {
    /// The time the list is computed for (the startup rule needs it).
    pub now: Timestamp,
    pub tickets: Vec<Ticket>,
    /// Subtasks of the cards that were left behind (`done_out_of_order.behind`).
    pub subtasks: Vec<Subtask>,
    pub threads: Vec<ThreadFacts>,
    /// The project's pins on its designs.
    pub pins: Vec<DesignPin>,
    /// Permission dialogs that runs reported and no card status shows.
    pub permissions: Vec<PendingPermission>,
    /// The live runs.
    pub runs: Vec<RunFacts>,
    /// The comments of the cards in review (the hand-in is read from them).
    pub comments: Vec<Comment>,
}

impl Default for InboxData {
    fn default() -> Self {
        Self {
            now: Timestamp(0),
            tickets: Vec::new(),
            subtasks: Vec::new(),
            threads: Vec::new(),
            pins: Vec::new(),
            permissions: Vec::new(),
            runs: Vec::new(),
            comments: Vec::new(),
        }
    }
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
    items.extend(question_items(data));
    items.extend(pins::pin_items(data));
    for i in items.iter_mut().filter(|i| i.kind == InboxKind::Review) {
        if let Some(t) = data
            .tickets
            .iter()
            .find(|t| Some(&t.key) == i.ticket.as_ref())
        {
            i.hand_in = hand_in::of_card(t, &data.comments);
        }
    }
    items.sort_by(|a, b| b.since.cmp(&a.since).then_with(|| a.ticket.cmp(&b.ticket)));
    items
}

fn item(kind: InboxKind, t: &Ticket, since: Timestamp, text: String) -> InboxItem {
    InboxItem {
        kind,
        ticket: Some(t.key.clone()),
        chain: Some(t.chain_id),
        project: None,
        title: t.title.clone(),
        since,
        text,
        detail: None,
        thread: None,
        subtasks: Vec::new(),
        done_cards: Vec::new(),
        link: t
            .pr
            .as_ref()
            .filter(|_| t.status == TicketStatus::Blocked)
            .map(|p| p.url.clone()),
        allowed_actions: TicketAction::allowed(t),
        role: None,
        answers: Vec::new(),
        choices: Vec::new(),
        lines: Vec::new(),
        pin: None,
        allow_always: None,
        allow_always_none: None,
        hand_in: None,
        design: None,
    }
}

/// The item a card makes by its own status, if any.
fn card_item(t: &Ticket) -> Option<InboxItem> {
    let (kind, text) = match (t.status, &t.status_reason) {
        (TicketStatus::Proposed, _) => (InboxKind::Proposal, String::new()),
        (
            TicketStatus::Waiting,
            Some(StatusReason::Waiting(
                WaitingReason::Question { text, .. } | WaitingReason::Permission { text, .. },
            )),
        ) => (InboxKind::Question, text.clone()),
        (
            TicketStatus::Blocked,
            Some(StatusReason::Blocked {
                reason:
                    BlockedReason::RunnerLost {
                        first: Some(_),
                        second: Some(_),
                    },
                ..
            }),
        ) => (InboxKind::Blocked, RUNNER_LOST_TEXT.to_owned()),
        (TicketStatus::Blocked, reason) => (
            InboxKind::Blocked,
            reason.as_ref().map(ToString::to_string).unwrap_or_default(),
        ),
        // The pull request has reported no check for the project's timeout: the person decides
        // (merge now, keep waiting, block); the card's actions say which of them are open.
        (TicketStatus::Review, _) if t.review.note == Some(ReviewNote::NoChecks) => {
            let url = t.pr.as_ref().map(|p| p.url.as_str()).unwrap_or_default();
            let mut i = item(
                InboxKind::Question,
                t,
                t.pr.as_ref()
                    .and_then(|p| p.no_checks_since)
                    .unwrap_or(t.status_since),
                format!("{NO_CHECKS_TEXT}: {url}"),
            );
            i.link = t.pr.as_ref().map(|p| p.url.clone());
            return Some(i);
        }
        // The reviewer agent still has the card (in agent mode it decides, in both mode the
        // person comes after its verdict), or the accepted work is being merged: nobody's turn.
        (TicketStatus::Review, _)
            if matches!(
                t.review.turn,
                Some(ReviewTurn::Check | ReviewTurn::Agent { .. } | ReviewTurn::Merging)
            ) =>
        {
            return None;
        }
        (TicketStatus::Review, _) => (InboxKind::Review, String::new()),
        _ => return None,
    };
    let mut i = item(kind, t, t.status_since, text);
    if kind == InboxKind::Review
        && let Some(ReviewNote::NoVerdict { lines }) = &t.review.note
    {
        NO_VERDICT_TEXT.clone_into(&mut i.text);
        i.lines.clone_from(lines);
    }
    if kind == InboxKind::Review
        && let Some(ReviewNote::CheckCannotRun { command }) = &t.review.note
    {
        i.text = crate::toolpath::cannot_run_text(command);
    }
    if kind == InboxKind::Question {
        i.role = Some(AgentRole::Worker);
        i.answers = match &t.status_reason {
            Some(StatusReason::Waiting(WaitingReason::Permission { .. })) => {
                QuestionKind::Permission.answers()
            }
            _ => QuestionKind::Question.answers(),
        };
        if let Some(StatusReason::Waiting(WaitingReason::Permission { detail, .. })) =
            &t.status_reason
        {
            i.detail.clone_from(detail);
            set_allow_always(&mut i);
        }
        if let Some(StatusReason::Waiting(WaitingReason::Question { choices, .. })) =
            &t.status_reason
        {
            i.choices.clone_from(choices);
        }
    }
    Some(i)
}

/// The questions that no card status shows: a permission dialog a run reported, and a run that
/// waits on its screen.
fn question_items(data: &InboxData) -> Vec<InboxItem> {
    let card = |id| data.tickets.iter().find(|t| t.id == id);
    let mut items = Vec::new();
    for p in &data.permissions {
        let Some(t) = card(p.ticket) else { continue };
        // The worker's dialog already makes the card Waiting, and the card lists it.
        if p.role == AgentRole::Worker && t.status == TicketStatus::Waiting {
            continue;
        }
        let mut i = item(InboxKind::Question, t, p.since, p.text.clone());
        i.role = Some(p.role);
        i.detail.clone_from(&p.detail);
        i.answers = QuestionKind::Permission.answers();
        if p.role == AgentRole::Worker {
            set_allow_always(&mut i);
        }
        items.push(i);
    }
    items.extend(screens::items(data));
    items
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
