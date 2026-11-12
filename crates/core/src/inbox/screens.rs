//! The inbox items of runs that wait on their own screen: what the screen shows and, when it is
//! Claude Code's trust dialog, the two answers it offers.

use super::{InboxData, InboxItem, InboxKind, item};
use crate::question::{self, AnswerKind, QuestionKind};
use crate::{AgentRole, StatusReason, TicketStatus, WaitingReason};

/// One `question` item per run that waits on its screen, for a card a person can still help: a
/// working card, or a card that waits on exactly this screen.
pub(super) fn items(data: &InboxData) -> Vec<InboxItem> {
    let mut items = Vec::new();
    for run in data.runs.iter().filter(|r| r.waits_on_screen(data.now)) {
        let Some(t) = data.tickets.iter().find(|t| t.id == run.ticket) else {
            continue;
        };
        let on_screen = t.status == TicketStatus::Waiting
            && t.status_reason == Some(StatusReason::Waiting(WaitingReason::Screen));
        let working = matches!(t.status, TicketStatus::Running | TicketStatus::Review) || on_screen;
        let asking = data
            .permissions
            .iter()
            .any(|p| p.ticket == run.ticket && p.role == run.role);
        if !working || asking || (run.role != AgentRole::Worker && on_screen) {
            continue;
        }
        let mut i = item(
            InboxKind::Question,
            t,
            run.waiting_since(),
            question::SCREEN_TEXT.to_owned(),
        );
        i.role = Some(run.role);
        i.lines = question::last_lines(&run.tail, question::SCREEN_LINES);
        i.answers = if question::is_trust_dialog(&i.lines) {
            vec![AnswerKind::Yes, AnswerKind::No]
        } else {
            QuestionKind::Screen.answers()
        };
        items.push(i);
    }
    items
}
