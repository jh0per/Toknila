//! What the daemon will do with an instruction no agent has received yet. Pure.

use serde::{Deserialize, Serialize};

use crate::{Ticket, TicketStatus};

/// Why a person's instruction is still pending, in what the daemon does about it.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PendingReason {
    /// An agent is being started for it.
    StartsAgent,
    /// Every slot of the project is busy; the agent starts when one is free.
    WaitsForSlot,
    /// The story has not started; the instruction is delivered when it does.
    AtStart,
}

/// The pending reason of a card, with the slots it may be waiting for.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingView {
    pub reason: PendingReason,
    /// Slots held now.
    pub busy: u32,
    /// The project's agent limit.
    pub slots: u32,
}

/// What happens to an instruction left on `ticket`. `queued` is whether the card waits for a
/// free slot; `alive` whether its agent has a process. `None` for a card whose agent is alive
/// (it gets the instruction at its next stop) or whose block a person has to look at.
pub fn pending_reason(ticket: &Ticket, queued: bool, alive: bool) -> Option<PendingReason> {
    let working = matches!(ticket.status, TicketStatus::Running | TicketStatus::Waiting);
    match ticket.status {
        TicketStatus::Proposed | TicketStatus::Backlog | TicketStatus::Ready => {
            Some(PendingReason::AtStart)
        }
        _ if queued => Some(PendingReason::WaitsForSlot),
        TicketStatus::Blocked if ticket.agent_paused.is_some() => Some(PendingReason::StartsAgent),
        // Paused, or its replacement run was just queued and has no process yet.
        _ if working && (ticket.agent_paused.is_some() || !alive) => {
            Some(PendingReason::StartsAgent)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ChainId, PausedAgent, TicketDraft, TicketId, Timestamp};

    fn card(status: TicketStatus) -> Ticket {
        let mut t = Ticket::new_backlog(
            TicketId::new(),
            ChainId::new(),
            "DEMO-1".parse().expect("key"),
            TicketDraft::default(),
            Timestamp(0),
        );
        t.status = status;
        t
    }

    #[test]
    fn each_state_of_a_card_without_an_agent_has_its_reason() {
        use PendingReason::*;
        assert_eq!(
            pending_reason(&card(TicketStatus::Ready), false, false),
            Some(AtStart)
        );
        assert_eq!(
            pending_reason(&card(TicketStatus::Running), false, false),
            Some(StartsAgent)
        );
        assert_eq!(
            pending_reason(&card(TicketStatus::Running), true, false),
            Some(WaitsForSlot)
        );
        let mut paused = card(TicketStatus::Waiting);
        paused.agent_paused = Some(PausedAgent {
            session: None,
            since: Timestamp(0),
            resume: true,
            lost: true,
        });
        assert_eq!(pending_reason(&paused, true, false), Some(WaitsForSlot));
        assert_eq!(pending_reason(&paused, false, false), Some(StartsAgent));
        // Control: a live agent gets it at its next stop.
        assert_eq!(
            pending_reason(&card(TicketStatus::Running), false, true),
            None
        );
    }
}
