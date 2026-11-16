//! The questions of the inbox that no card status of its own shows (a run's permission dialog, a
//! screen), and the row of a question a person answered whose delivery waits for a slot.

use super::{InboxData, InboxItem, InboxKind, item, screens, set_allow_always};
use crate::question::QuestionKind;
use crate::{AgentRole, StatusReason, Ticket, TicketStatus, WaitingReason};

/// The informational row of a question, permission or approval that a person answered while the
/// card's agent has no process: the answer is delivered when the agent starts again, and a start
/// waits for a free slot. It needs nothing from the person.
pub(super) fn answered(t: &Ticket, data: &InboxData) -> Option<InboxItem> {
    let waiting = matches!(
        &t.status_reason,
        Some(StatusReason::Waiting(
            WaitingReason::Question { .. } | WaitingReason::Permission { .. }
        ))
    );
    if t.status != TicketStatus::Waiting || !waiting || !t.agent_paused.as_ref()?.resume {
        return None;
    }
    let place = data
        .positions
        .iter()
        .find(|(id, _)| *id == t.id)
        .map(|(_, n)| format!(" (#{n} in line)"))
        .unwrap_or_default();
    let text = format!("Answered; the agent gets it when a slot is free{place}");
    let mut i = item(InboxKind::Answered, t, t.status_since, text);
    i.needs_human = false;
    i.allowed_actions = Vec::new();
    i.role = Some(AgentRole::Worker);
    Some(i)
}

/// The questions that no card status shows: a permission dialog a run reported, and a run that
/// waits on its screen.
pub(super) fn items(data: &InboxData) -> Vec<InboxItem> {
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
