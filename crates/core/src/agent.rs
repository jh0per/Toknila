//! The agent event contract: what a running agent reports about its ticket.
//!
//! Phase 0's `sim` profile receives these from the CLI (`tok sim ...`); phase 1 sends the same
//! events from MCP tools and agent hooks. See `docs/events.md`.

use serde::{Deserialize, Serialize};

use crate::{CommentBody, WaitingReason};

/// One report from the agent working on a ticket.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum AgentEvent {
    /// The runner is up and the agent is working. Starts a Ready ticket; no-op if Running.
    Started,
    /// The agent needs a person: a question, or permission for an action.
    AskHuman {
        text: String,
        #[serde(default)]
        permission: bool,
    },
    /// The agent ticked a checklist item (1-based position).
    SubtaskDone { position: u32 },
    /// A short progress report, stored as a report comment.
    Note { body: CommentBody },
    /// The agent is done; the summary becomes a report comment and the ticket goes to Review.
    Finished { summary: String },
    /// The agent process died.
    Crashed { detail: String },
}

impl AgentEvent {
    /// The question as a Waiting reason, for `AskHuman`.
    pub fn waiting_reason(text: &str, permission: bool) -> WaitingReason {
        if permission {
            WaitingReason::Permission {
                text: text.to_owned(),
            }
        } else {
            WaitingReason::Question {
                text: text.to_owned(),
            }
        }
    }
}

/// What a release agent reports about its milestone.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum ReleaseEvent {
    /// Built, shipped to the channel, merged and tagged: the milestone is released.
    Finished,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_shape_is_pinned() {
        let cases = [
            (AgentEvent::Started, r#"{"event":"started"}"#),
            (
                AgentEvent::AskHuman {
                    text: "який фреймворк?".into(),
                    permission: false,
                },
                r#"{"event":"ask_human","text":"який фреймворк?","permission":false}"#,
            ),
            (
                AgentEvent::SubtaskDone { position: 1 },
                r#"{"event":"subtask_done","position":1}"#,
            ),
            (
                AgentEvent::Note {
                    body: CommentBody {
                        gist: "half way".into(),
                        ..CommentBody::default()
                    },
                },
                r#"{"event":"note","body":{"gist":"half way"}}"#,
            ),
            (
                AgentEvent::Finished {
                    summary: "зроблено X".into(),
                },
                r#"{"event":"finished","summary":"зроблено X"}"#,
            ),
            (
                AgentEvent::Crashed {
                    detail: "exit 101".into(),
                },
                r#"{"event":"crashed","detail":"exit 101"}"#,
            ),
        ];
        for (event, json) in cases {
            assert_eq!(serde_json::to_string(&event).expect("serialize"), json);
            assert_eq!(
                serde_json::from_str::<AgentEvent>(json).expect("parse"),
                event
            );
        }
        let ask: AgentEvent =
            serde_json::from_str(r#"{"event":"ask_human","text":"?"}"#).expect("parse");
        assert_eq!(
            ask,
            AgentEvent::AskHuman {
                text: "?".into(),
                permission: false
            },
            "permission defaults to false"
        );
    }
}
