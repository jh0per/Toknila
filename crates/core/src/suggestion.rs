//! The planner's standing suggestion: the one next step it tells the owner, kept per project.
//! A new one replaces the old; it ends when its action is done or its card moves on. The
//! person clicks the button of the action; the planner never does.

use serde::{Deserialize, Serialize};

use crate::events::DomainEvent;
use crate::machine::TicketAction;
use crate::{ChainId, ProjectKey, Ticket, TicketKey, TicketStatus, Timestamp};

/// What the button of a suggestion does.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SuggestionAction {
    /// Start the card.
    Start,
    /// Answer the card's question.
    Answer,
    /// Accept the card's work.
    Accept,
    /// Release the chain the card is in.
    Release,
}

impl SuggestionAction {
    /// The name on the wire.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Answer => "answer",
            Self::Accept => "accept",
            Self::Release => "release",
        }
    }
}

impl std::str::FromStr for SuggestionAction {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        [Self::Start, Self::Answer, Self::Accept, Self::Release]
            .into_iter()
            .find(|a| a.as_str() == s)
            .ok_or_else(|| format!("action {s:?}: use start, answer, accept or release"))
    }
}

/// A project's standing suggestion.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Suggestion {
    pub project: ProjectKey,
    /// What the planner says, in a line or two. With no card and no action it says that nothing
    /// waits for the person.
    pub text: String,
    /// The card it is about.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub card: Option<TicketKey>,
    /// The chain of the card, so a client can act on the chain (release).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chain: Option<ChainId>,
    /// The one action its button does.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<SuggestionAction>,
    pub at: Timestamp,
}

/// Why a suggestion is refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SuggestionRefusal {
    #[error("a suggestion needs a text")]
    NoText,
    #[error("an action needs the story it is about")]
    ActionNeedsCard,
    #[error("a story needs the action to do with it; to only point at a story use show_card")]
    CardNeedsAction,
    #[error("{card} is {status}: {action} does not apply to it now")]
    NotNow {
        card: TicketKey,
        status: String,
        action: &'static str,
    },
}

/// Checks a suggestion before it is kept: a text, an action with its card (or neither), and an
/// action the card allows now.
pub fn check(
    text: &str,
    action: Option<SuggestionAction>,
    card: Option<&Ticket>,
) -> Result<(), SuggestionRefusal> {
    if text.trim().is_empty() {
        return Err(SuggestionRefusal::NoText);
    }
    let (action, card) = match (action, card) {
        (None, None) => return Ok(()),
        (Some(_), None) => return Err(SuggestionRefusal::ActionNeedsCard),
        (None, Some(_)) => return Err(SuggestionRefusal::CardNeedsAction),
        (Some(a), Some(c)) => (a, c),
    };
    let applies = match action {
        SuggestionAction::Start => TicketAction::allowed(card).contains(&TicketAction::Start),
        SuggestionAction::Answer => card.status == TicketStatus::Waiting,
        SuggestionAction::Accept => TicketAction::allowed(card).contains(&TicketAction::Accept),
        SuggestionAction::Release => true,
    };
    if applies {
        Ok(())
    } else {
        Err(SuggestionRefusal::NotNow {
            card: card.key.clone(),
            status: crate::wire::wire_name(&card.status).unwrap_or_default(),
            action: action.as_str(),
        })
    }
}

/// What an event ends: the suggestions about a card, or the release suggestions of a chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ends {
    /// The card's status moved: it was started, answered, accepted, or moved on some other way.
    Card(TicketKey),
    /// A release of the chain was made or moved.
    Release(ChainId),
}

/// What `event` ends, if it ends any suggestion.
pub fn ends(event: &DomainEvent) -> Option<Ends> {
    match event {
        DomainEvent::TicketStatusChanged { key, .. } => Some(Ends::Card(key.clone())),
        DomainEvent::ReleaseCreated { release }
        | DomainEvent::ReleaseStateChanged { release, .. } => Some(Ends::Release(release.chain_id)),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
