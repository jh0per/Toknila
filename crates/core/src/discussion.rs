//! A card's discussion with the project's assistant: the rules of its edit proposals and what the
//! card's agent reads of it. A discussion is a thread of kind `discussion`; a proposal is a
//! message of kind `edit_proposal` whose text is an [`EditProposal`] as JSON, and a person's
//! decision is a later message that names it. Pure.

use serde::{Deserialize, Serialize};

use crate::{Actor, MessageKind, ThreadMessage};

/// The title of a card's discussion thread.
pub const TITLE: &str = "Talk to the assistant";

/// One field as it was and as the assistant proposes it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Change<T> {
    pub from: T,
    pub to: T,
}

/// The change of a card's prompt and/or acceptance the assistant proposes.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditProposal {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<Change<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acceptance: Option<Change<Vec<String>>>,
}

impl EditProposal {
    /// Whether it changes nothing.
    pub fn is_empty(&self) -> bool {
        self.prompt.is_none() && self.acceptance.is_none()
    }

    /// The proposal for `prompt` and `acceptance` against the card as it is now: only what
    /// differs.
    pub fn of(
        now_prompt: &str,
        now_acceptance: &[String],
        prompt: Option<String>,
        acceptance: Option<Vec<String>>,
    ) -> Self {
        Self {
            prompt: prompt.filter(|p| p != now_prompt).map(|to| Change {
                from: now_prompt.to_owned(),
                to,
            }),
            acceptance: acceptance
                .filter(|a| a.as_slice() != now_acceptance)
                .map(|to| Change {
                    from: now_acceptance.to_vec(),
                    to,
                }),
        }
    }

    /// Whether the card still has what the proposal was made against.
    pub fn applies_to(&self, now_prompt: &str, now_acceptance: &[String]) -> bool {
        self.prompt.as_ref().is_none_or(|c| c.from == now_prompt)
            && self
                .acceptance
                .as_ref()
                .is_none_or(|c| c.from.as_slice() == now_acceptance)
    }
}

/// Where a proposal stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Pending,
    Accepted,
    Rejected,
}

/// The state of the proposal `message` given the thread's messages.
pub fn decision(messages: &[ThreadMessage], message: &ThreadMessage) -> Decision {
    let id = message.id.to_string();
    let decided = |kind| {
        messages
            .iter()
            .any(|m| m.kind == kind && m.text == id && m.created_at >= message.created_at)
    };
    if decided(MessageKind::EditAccepted) {
        Decision::Accepted
    } else if decided(MessageKind::EditRejected) {
        Decision::Rejected
    } else {
        Decision::Pending
    }
}

/// The proposal in a message, if it is one that reads.
pub fn proposal_of(message: &ThreadMessage) -> Option<EditProposal> {
    (message.kind == MessageKind::EditProposal)
        .then(|| serde_json::from_str(&message.text).ok())
        .flatten()
}

fn list(items: &[String]) -> String {
    if items.is_empty() {
        return "(none)".to_owned();
    }
    items
        .iter()
        .map(|i| format!("\"{}\"", i.trim()))
        .collect::<Vec<_>>()
        .join("; ")
}

/// What the card's agent reads of a discussion, oldest first: who said what, and what became of
/// each proposed edit.
pub fn transcript(messages: &[ThreadMessage]) -> Vec<String> {
    let mut out = Vec::new();
    for m in messages {
        let who = match m.author {
            Actor::Human => "person",
            Actor::Agent => "assistant",
            Actor::System => "toknila",
        };
        match m.kind {
            MessageKind::Message | MessageKind::ChangesRequested => {
                out.push(format!("{who}: {}", one_line(&m.text)));
            }
            MessageKind::EditProposal => {
                let Some(p) = proposal_of(m) else { continue };
                let state = match decision(messages, m) {
                    Decision::Pending => "waiting for the person",
                    Decision::Accepted => "accepted: the story above has it",
                    Decision::Rejected => "rejected: the story is unchanged",
                };
                let mut line = format!("assistant proposed an edit ({state})");
                if let Some(c) = &p.prompt {
                    line.push_str(&format!("; prompt to: {}", one_line(&c.to)));
                }
                if let Some(c) = &p.acceptance {
                    line.push_str(&format!("; acceptance to: {}", list(&c.to)));
                }
                out.push(line);
            }
            MessageKind::EditAccepted | MessageKind::EditRejected => {}
        }
    }
    out
}

/// How long a person's message may go without an answer before the box offers a nudge and the
/// daemon logs a warning.
pub const ANSWER_WAIT_SECS: u64 = 90;

/// The person's message nobody has answered: the newest message or proposal is theirs.
pub fn unanswered(messages: &[ThreadMessage]) -> Option<&ThreadMessage> {
    messages
        .iter()
        .rfind(|m| matches!(m.kind, MessageKind::Message | MessageKind::EditProposal))
        .filter(|m| m.author == Actor::Human)
}

fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests;
