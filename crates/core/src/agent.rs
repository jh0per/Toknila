//! The agent event contract: what a running agent reports about its ticket.
//!
//! Phase 0's `sim` profile receives these from the CLI (`tok sim ...`); phase 1 sends the same
//! events from MCP tools and agent hooks. See `docs/events.md`.

use serde::{Deserialize, Serialize};

use crate::{ArtifactFormat, CommentBody, ThreadKind, WaitingReason};

/// A wildcard card as proposed: work nobody planned, found during a ticket. References are
/// ticket keys, titles or release versions of the same chain.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Proposal {
    pub title: String,
    /// The task for whoever does it.
    #[serde(default)]
    pub prompt: String,
    /// Acceptance criteria, one per entry.
    #[serde(default)]
    pub acceptance: Vec<String>,
    /// Nodes the card waits for.
    #[serde(default)]
    pub after: Vec<String>,
    /// Nodes that wait for the card; none of them may have started when it is approved.
    #[serde(default)]
    pub before: Vec<String>,
}

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
    /// The agent found work nobody planned: a wildcard card on the ticket's chain, found during
    /// this ticket, that waits for a person's approval. The answer is the new card.
    Propose(Proposal),
    /// The agent opens a thread to settle a design, a flow or an architecture with the owner.
    /// The hand-in is refused until every thread is approved.
    OpenThread { kind: ThreadKind, title: String },
    /// A message from the agent in a thread (`thread`: its number on this ticket, or its id).
    ThreadMessage { thread: String, text: String },
    /// A new artifact version from the agent. A new version reopens an approved thread.
    ThreadVersion {
        thread: String,
        format: ArtifactFormat,
        body: String,
    },
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
            (
                AgentEvent::Propose(Proposal {
                    title: "token refresh".into(),
                    prompt: "add the endpoint".into(),
                    acceptance: vec!["401 refreshes".into()],
                    after: vec!["DEMO-1".into()],
                    before: vec!["DEMO-2".into()],
                }),
                concat!(
                    r#"{"event":"propose","title":"token refresh","prompt":"add the endpoint","#,
                    r#""acceptance":["401 refreshes"],"after":["DEMO-1"],"before":["DEMO-2"]}"#
                ),
            ),
            (
                AgentEvent::OpenThread {
                    kind: ThreadKind::Design,
                    title: "List screen".into(),
                },
                r#"{"event":"open_thread","kind":"design","title":"List screen"}"#,
            ),
            (
                AgentEvent::ThreadMessage {
                    thread: "1".into(),
                    text: "empty state?".into(),
                },
                r#"{"event":"thread_message","thread":"1","text":"empty state?"}"#,
            ),
            (
                AgentEvent::ThreadVersion {
                    thread: "1".into(),
                    format: ArtifactFormat::Mermaid,
                    body: "graph TD; A-->B".into(),
                },
                r#"{"event":"thread_version","thread":"1","format":"mermaid","body":"graph TD; A-->B"}"#,
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
        let propose: AgentEvent =
            serde_json::from_str(r#"{"event":"propose","title":"x"}"#).expect("parse");
        assert_eq!(
            propose,
            AgentEvent::Propose(Proposal {
                title: "x".into(),
                ..Proposal::default()
            }),
            "only the title is required"
        );
    }
}
