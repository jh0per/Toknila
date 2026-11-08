//! What a person can do with a ticket, as named actions (`POST /tickets/{t}/actions/{action}`,
//! the CLI verbs, the buttons of the UI). Which actions a ticket allows now is computed by
//! trying each against the transition table, so no client keeps a copy of the rules.

use std::str::FromStr;

use serde::{Deserialize, Serialize};

use super::{TicketEvent, TransitionError, apply};
use crate::{Actor, BlockedReason, Ticket, Timestamp};

/// What a person can do with a ticket.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TicketAction {
    Start,
    Pause,
    Resume,
    /// Needs a text.
    Block,
    Unblock,
    /// Move the base branch to its upstream, then start (a card blocked by a base that is behind).
    FastForwardStart,
    /// Start from the base branch as it is (a card blocked by a base that is behind).
    StartAnyway,
    /// Merge the work; remarks are optional.
    Accept,
    /// Send the work back with a comment (a text is required).
    Return,
    Reopen,
    /// Needs a text.
    Cancel,
    /// Let a wildcard card into the plan.
    Approve,
    /// Turn a wildcard card down, with a text.
    Reject,
    /// Run the reviewer again, after it gave no verdict.
    RunReviewer,
    /// Ask for a design to approve before any code (`tok design`).
    AskDesign,
}

/// The name is not an action.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("action {0}")]
pub struct UnknownAction(pub String);

impl FromStr for TicketAction {
    type Err = UnknownAction;

    fn from_str(s: &str) -> Result<Self, UnknownAction> {
        Self::ALL
            .into_iter()
            .find(|a| a.name() == s)
            .ok_or_else(|| UnknownAction(s.to_owned()))
    }
}

impl TicketAction {
    /// Every action, in the order a client shows them.
    pub const ALL: [TicketAction; 15] = [
        Self::Start,
        Self::Pause,
        Self::Resume,
        Self::Block,
        Self::Unblock,
        Self::FastForwardStart,
        Self::StartAnyway,
        Self::Accept,
        Self::Return,
        Self::Reopen,
        Self::AskDesign,
        Self::Cancel,
        Self::Approve,
        Self::Reject,
        Self::RunReviewer,
    ];

    /// The name used in routes and commands.
    pub fn name(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Pause => "pause",
            Self::Resume => "resume",
            Self::Block => "block",
            Self::Unblock => "unblock",
            Self::FastForwardStart => "fast_forward_start",
            Self::StartAnyway => "start_anyway",
            Self::Accept => "accept",
            Self::Return => "return",
            Self::Reopen => "reopen",
            Self::Cancel => "cancel",
            Self::Approve => "approve",
            Self::Reject => "reject",
            Self::RunReviewer => "run_reviewer",
            Self::AskDesign => "ask_design",
        }
    }

    /// The state-machine event for this action, issued `by` the command's actor; block, return,
    /// cancel and reject need a text.
    pub fn event(self, text: Option<String>, by: Actor) -> Result<TicketEvent, TransitionError> {
        let text = text.filter(|t| !t.trim().is_empty());
        let required = |what: &'static str| text.clone().ok_or(TransitionError::EmptyText(what));
        Ok(match self {
            Self::Start => TicketEvent::Request,
            Self::Pause => TicketEvent::Pause,
            Self::Resume => TicketEvent::Resume,
            Self::Block => TicketEvent::Block {
                reason: BlockedReason::Manual {
                    text: required("block")?,
                },
            },
            Self::Unblock => TicketEvent::Unblock,
            Self::FastForwardStart => TicketEvent::FastForwardStart,
            Self::StartAnyway => TicketEvent::StartAnyway,
            Self::Accept => TicketEvent::Accept {
                comment: text.clone(),
            },
            Self::Return => TicketEvent::Return {
                comment: required("return")?,
            },
            Self::Reopen => TicketEvent::Reopen,
            Self::Cancel => TicketEvent::Cancel {
                reason: required("cancel")?,
            },
            Self::Approve => TicketEvent::Approve { by },
            Self::Reject => TicketEvent::Reject {
                reason: required("reject")?,
                by,
            },
            Self::RunReviewer => TicketEvent::RunReviewer,
            Self::AskDesign => TicketEvent::AskDesign,
        })
    }

    /// The actions a person may take with `ticket` now: those its status lets through the
    /// transition table (with a text, for the ones that need one).
    pub fn allowed(ticket: &Ticket) -> Vec<TicketAction> {
        Self::ALL
            .into_iter()
            .filter(|a| {
                a.event(Some("-".to_owned()), Actor::Human)
                    .and_then(|e| apply(ticket, &e, Timestamp(0)))
                    .is_ok()
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ChainId, ReviewMode, TicketDraft, TicketId, TicketStatus};

    fn ticket(status: TicketStatus) -> Ticket {
        let mut t = Ticket::new_backlog(
            TicketId::new(),
            ChainId::new(),
            "DEMO-1".parse().expect("key"),
            TicketDraft::default(),
            Timestamp(1),
        );
        t.status = status;
        t
    }

    fn names(t: &Ticket) -> Vec<&'static str> {
        TicketAction::allowed(t)
            .into_iter()
            .map(TicketAction::name)
            .collect()
    }

    #[test]
    fn a_card_in_review_can_be_accepted_or_returned() {
        let mut t = ticket(TicketStatus::Review);
        t.review.turn = Some(ReviewMode::Human.first_turn());
        assert_eq!(names(&t), ["accept", "return", "cancel"]);
    }

    #[test]
    fn what_is_allowed_follows_the_status() {
        assert_eq!(
            names(&ticket(TicketStatus::Ready)),
            ["start", "pause", "ask_design", "cancel"]
        );
        assert_eq!(
            names(&ticket(TicketStatus::Running)),
            ["pause", "block", "ask_design", "cancel"]
        );
        assert_eq!(names(&ticket(TicketStatus::Merged)), ["reopen"]);
        assert!(names(&ticket(TicketStatus::Released)).is_empty());
    }

    #[test]
    fn names_round_trip_and_unknown_names_are_refused() {
        for a in TicketAction::ALL {
            assert_eq!(a.name().parse::<TicketAction>(), Ok(a));
        }
        assert_eq!(
            "explode".parse::<TicketAction>(),
            Err(UnknownAction("explode".into()))
        );
    }

    #[test]
    fn accept_takes_remarks_but_return_needs_a_text() {
        let accept = TicketAction::Accept.event(Some(" nice ".into()), Actor::Human);
        assert_eq!(
            accept,
            Ok(TicketEvent::Accept {
                comment: Some(" nice ".into())
            })
        );
        assert_eq!(
            TicketAction::Accept.event(None, Actor::Human),
            Ok(TicketEvent::Accept { comment: None })
        );
        assert_eq!(
            TicketAction::Return.event(Some("  ".into()), Actor::Human),
            Err(TransitionError::EmptyText("return"))
        );
    }
}
