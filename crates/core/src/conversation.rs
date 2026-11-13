//! A project's conversation with its assistant: what a message is and who may write which
//! author. The record is the chat the person reads; the assistant's terminal is its own work.
//! Pure.
//!
//! Three authors write it: the person (from the assistant's screen, or on a card's discussion),
//! the assistant (its `say` tool, or an answer in a card's discussion) and Toknila itself (a line
//! about something the assistant did). A card's discussion lives once, in the card's thread; the
//! conversation reads it as a message with the card as its context.

use serde::{Deserialize, Serialize};

use crate::{Actor, TicketKey, Timestamp};

/// How many messages a page of the conversation holds when the caller does not say.
pub const PAGE: usize = 50;

/// The most messages one page may hold.
pub const MAX_PAGE: usize = 200;

/// The longest text of one message, in characters.
pub const MAX_TEXT: usize = 20_000;

/// A fresh id for a message the conversation stores. Ids grow with every call (and with every
/// id of any other entity made in this process), also within one millisecond, so sorting by
/// (time, id) lists messages, stored or from a thread, in the order they were said.
pub fn next_id() -> String {
    crate::model::next_ulid().to_string()
}

/// Who wrote a message.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Author {
    /// The owner.
    Person,
    /// The project's planner.
    Assistant,
    /// The daemon, reporting what the assistant did.
    Toknila,
}

impl Author {
    /// The name in the store.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Person => "person",
            Self::Assistant => "assistant",
            Self::Toknila => "toknila",
        }
    }

    /// The author of a message that a card's thread holds, by who posted it there.
    pub fn of_thread_author(actor: Actor) -> Self {
        match actor {
            Actor::Human => Self::Person,
            Actor::Agent | Actor::Assistant => Self::Assistant,
            Actor::System => Self::Toknila,
        }
    }
}

impl std::str::FromStr for Author {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        [Self::Person, Self::Assistant, Self::Toknila]
            .into_iter()
            .find(|a| a.as_str() == s)
            .ok_or_else(|| format!("author {s:?}: use person, assistant or toknila"))
    }
}

/// What a message was about.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum About {
    /// A card, by key.
    Card { key: TicketKey },
    /// A chain (epic), by name; the window shows the chain's title.
    Chain { name: String },
    /// The roadmap: the project's top-level plan of epics.
    Roadmap,
}

impl About {
    /// A message about this chain.
    pub fn chain_of(c: &crate::Chain) -> Self {
        Self::Chain {
            name: c.name.clone(),
        }
    }
}

/// One message of a project's conversation.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    /// Unique across the whole conversation, including a card discussion's messages.
    pub id: String,
    pub author: Author,
    pub text: String,
    /// The card or chain it was about.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub about: Option<About>,
    /// Names of the files that went with it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<String>,
    /// For a message of a card's discussion: the number of the thread it lives in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread: Option<u32>,
    pub at: Timestamp,
}

/// Why a message is refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Refusal {
    /// Nothing to say.
    #[error("a message needs some text")]
    Empty,
    /// More than a message may hold.
    #[error("a message holds at most {MAX_TEXT} characters")]
    TooLong,
    /// The caller is not the kind of writer that author needs.
    #[error("only {needs} can write as {author}")]
    NotYours {
        author: &'static str,
        needs: &'static str,
    },
}

/// The text of a message, trimmed; refused when empty or too long.
pub fn clean_text(text: &str) -> Result<String, Refusal> {
    let text = text.trim();
    if text.is_empty() {
        return Err(Refusal::Empty);
    }
    if text.chars().count() > MAX_TEXT {
        return Err(Refusal::TooLong);
    }
    Ok(text.to_owned())
}

/// Whether `actor` may write a message as `author`: the person writes as the person, the planner
/// (an agent) as the assistant. Toknila's own lines have no caller: only the daemon writes them.
pub fn check_writer(actor: Actor, author: Author) -> Result<(), Refusal> {
    match (author, actor) {
        (Author::Person, Actor::Human) | (Author::Assistant, Actor::Agent) => Ok(()),
        (Author::Person, _) => Err(Refusal::NotYours {
            author: "the person",
            needs: "a person",
        }),
        (Author::Assistant, _) => Err(Refusal::NotYours {
            author: "the assistant",
            needs: "the assistant",
        }),
        (Author::Toknila, _) => Err(Refusal::NotYours {
            author: "Toknila",
            needs: "the daemon itself",
        }),
    }
}

/// Number of messages to read for a page asked with `limit`: the default when unset, never more
/// than [`MAX_PAGE`], at least one.
pub fn page_size(limit: Option<usize>) -> usize {
    limit.unwrap_or(PAGE).clamp(1, MAX_PAGE)
}

/// What Toknila says when the assistant added a card.
pub fn added_card(key: &TicketKey) -> String {
    format!("added story {key}")
}

/// What Toknila says when the assistant edited a card.
pub fn edited_card(key: &TicketKey) -> String {
    format!("edited {key}")
}

/// What Toknila says when the assistant proposed a change that the person has yet to accept.
pub fn proposed_edit(key: &TicketKey) -> String {
    format!("proposed a change of {key}")
}

/// What Toknila says when the assistant opened a thread on a card.
pub fn opened_thread(number: u32, key: &TicketKey) -> String {
    format!("opened thread {number} on {key}")
}

/// What Toknila says when the assistant added an epic (a chain) with its release version; `title`
/// is what a person reads.
pub fn added_chain(title: &str, version: Option<&str>) -> String {
    match version.map(|v| v.trim().trim_start_matches('v')) {
        Some(v) if !v.is_empty() => format!("added epic {title} v{v}"),
        _ => format!("added epic {title}"),
    }
}

/// What Toknila says when the assistant edited a chain.
pub fn edited_chain(title: &str) -> String {
    format!("edited epic {title}")
}

/// What Toknila says when the assistant set its suggestion.
pub fn suggested(text: &str) -> String {
    format!("suggested: {text}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_person_writes_as_the_person_and_only_the_planner_as_the_assistant() {
        assert!(check_writer(Actor::Human, Author::Person).is_ok());
        assert!(check_writer(Actor::Agent, Author::Assistant).is_ok());
        for (actor, author) in [
            (Actor::Agent, Author::Person),
            (Actor::System, Author::Person),
            (Actor::Human, Author::Assistant),
            (Actor::System, Author::Assistant),
            (Actor::Human, Author::Toknila),
            (Actor::Agent, Author::Toknila),
            (Actor::System, Author::Toknila),
        ] {
            assert!(
                check_writer(actor, author).is_err(),
                "{actor:?} as {author:?}"
            );
        }
    }

    #[test]
    fn a_text_is_trimmed_and_must_be_there_and_fit() {
        assert_eq!(clean_text("  hi \n"), Ok("hi".to_owned()));
        assert_eq!(clean_text(" \n "), Err(Refusal::Empty));
        assert_eq!(clean_text(&"x".repeat(MAX_TEXT + 1)), Err(Refusal::TooLong));
        assert!(clean_text(&"x".repeat(MAX_TEXT)).is_ok());
    }

    #[test]
    fn ids_grow_with_every_call_so_they_order_messages_said_in_one_millisecond() {
        let ids: Vec<String> = (0..2000).map(|_| next_id()).collect();
        let mut sorted = ids.clone();
        sorted.sort();
        assert_eq!(ids, sorted);
        sorted.dedup();
        assert_eq!(sorted.len(), ids.len(), "no id twice");
    }

    #[test]
    fn a_page_is_fifty_by_default_and_never_more_than_the_maximum() {
        assert_eq!(page_size(None), 50);
        assert_eq!(page_size(Some(0)), 1);
        assert_eq!(page_size(Some(10_000)), MAX_PAGE);
    }

    #[test]
    fn the_message_is_pinned_on_the_wire() {
        let m = Message {
            id: "01JBQ7ZJ0000000000000000M1".into(),
            author: Author::Toknila,
            text: "added story DEMO-2".into(),
            about: Some(About::Card {
                key: "DEMO-2".parse().expect("key"),
            }),
            attachments: vec![],
            thread: None,
            at: Timestamp(5),
        };
        assert_eq!(
            serde_json::to_value(&m).expect("json"),
            serde_json::json!({
                "id": "01JBQ7ZJ0000000000000000M1", "author": "toknila",
                "text": "added story DEMO-2",
                "about": { "kind": "card", "key": "DEMO-2" }, "at": 5
            })
        );
        let chain = About::Chain {
            name: "plan".into(),
        };
        assert_eq!(
            serde_json::to_value(&chain).expect("json"),
            serde_json::json!({ "kind": "chain", "name": "plan" })
        );
        assert_eq!(
            serde_json::to_value(About::Roadmap).expect("json"),
            serde_json::json!({ "kind": "roadmap" })
        );
        assert_eq!(added_chain("plan", Some("0.4.0")), "added epic plan v0.4.0");
        assert_eq!(added_chain("plan", None), "added epic plan");
    }
}
