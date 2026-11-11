//! The agent event contract: what a running agent reports about its ticket.
//!
//! The `sim` profile receives these from the CLI (`tok sim ...`); real agents send the same
//! events from MCP tools and agent hooks. See `docs/events.md`.

use serde::{Deserialize, Serialize};

use crate::{ArtifactFormat, CommentBody, ThreadKind, Verdict, WaitingReason};

/// A wildcard card as proposed: work nobody planned, found during a ticket. References are
/// ticket keys, titles or release versions of the same chain.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
#[serde(tag = "event", rename_all = "snake_case", deny_unknown_fields)]
pub enum AgentEvent {
    /// The runner is up and the agent is working. Starts a Ready ticket; no-op if Running.
    Started,
    /// The agent needs a person: a question, or permission for an action.
    AskHuman {
        text: String,
        #[serde(default)]
        permission: bool,
        /// The whole command a permission request is about, when `text` is only its summary.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        detail: Option<String>,
    },
    /// The agent ticked a checklist item (1-based position).
    SubtaskDone { position: u32 },
    /// A short progress report, stored as a report comment.
    Note { body: CommentBody },
    /// The agent is done; the summary becomes a report comment and the ticket goes to Review.
    Finished { summary: String },
    /// The agent process died.
    Crashed { detail: String },
    /// Only the reviewer agent's token may send this: its decision about the diff. The comment
    /// says what to change and is required with `request_changes`.
    ReviewVerdict {
        verdict: Verdict,
        #[serde(default)]
        comment: String,
        /// PASS or FAIL for each acceptance item of the ticket (required when it has any).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        items: Vec<crate::ItemVerdict>,
    },
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
    /// The agent takes back its own thread while it has no version and no person message.
    WithdrawThread { thread: String },
}

impl AgentEvent {
    /// Whether this is the reviewer's verdict: only a reviewer sends it, and sends nothing else.
    pub fn is_review_verdict(&self) -> bool {
        matches!(self, Self::ReviewVerdict { .. })
    }

    /// The text of a note that asks a question: it ends with a question mark, or asks "which",
    /// "may I" or "should I". Such a note is a question in disguise; `None` for a plain report.
    pub fn note_question(body: &CommentBody) -> Option<String> {
        let mut text = body.gist.trim().to_owned();
        if let Some(what) = body.what.as_deref().filter(|w| !w.trim().is_empty()) {
            text.push('\n');
            text.push_str(what.trim());
        }
        let lower = text.to_lowercase();
        let words: Vec<&str> = lower
            .split(|c: char| !c.is_alphanumeric() && c != '\'')
            .filter(|w| !w.is_empty())
            .collect();
        // "which" asks only where a sentence starts with it, not as a relative clause.
        let which_starts = lower.split(['.', '!', '?', '\n']).any(|sentence| {
            let sentence = sentence.trim_start_matches(|c: char| !c.is_alphanumeric());
            sentence.split(|c: char| !c.is_alphanumeric()).next() == Some("which")
        });
        let asks = text.ends_with('?')
            || which_starts
            || words
                .windows(2)
                .any(|w| w == ["may", "i"] || w == ["should", "i"]);
        (asks && !text.is_empty()).then_some(text)
    }

    /// What a card's agent is told when its note was turned into a question.
    pub const NOTE_ASKED: &'static str = "Your note asked a question, so Toknila put it in the inbox as a question and the card now waits. A note is a report and is never answered: to ask a person something, use ask_human.";

    /// The question as a Waiting reason, for `AskHuman`.
    pub fn waiting_reason(text: &str, permission: bool, detail: Option<String>) -> WaitingReason {
        if permission {
            WaitingReason::Permission {
                text: text.to_owned(),
                detail,
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
#[serde(tag = "event", rename_all = "snake_case", deny_unknown_fields)]
pub enum ReleaseEvent {
    /// Built, shipped to the channel, merged and tagged: the milestone is released.
    Finished,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(gist: &str, what: Option<&str>) -> CommentBody {
        CommentBody {
            gist: gist.into(),
            what: what.map(Into::into),
            ..CommentBody::default()
        }
    }

    #[test]
    fn a_note_that_asks_is_a_question_and_a_progress_note_is_not() {
        for asks in [
            "Is the parser meant to accept tabs?",
            "Which framework should we use",
            "May I delete the old module",
            "I wonder, should I split this card",
            "Parser done. Which framework next",
        ] {
            assert_eq!(
                AgentEvent::note_question(&note(asks, None)).as_deref(),
                Some(asks)
            );
        }
        let long = note("Parser question", Some("Tabs or spaces?"));
        assert_eq!(
            AgentEvent::note_question(&long).as_deref(),
            Some("Parser question\nTabs or spaces?")
        );
        for plain in [
            "Parser done, 12 tests pass",
            "Why this works: the cache is keyed by id.",
            "I may inspect the code next",
            "The parser, which handles tabs, is done",
            "Tests pass. The module, which is small, is committed",
        ] {
            assert_eq!(
                AgentEvent::note_question(&note(plain, None)),
                None,
                "{plain}"
            );
        }
    }

    #[test]
    fn wire_shape_is_pinned() {
        let cases = [
            (AgentEvent::Started, r#"{"event":"started"}"#),
            (
                AgentEvent::AskHuman {
                    text: "який фреймворк?".into(),
                    permission: false,
                    detail: None,
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
                AgentEvent::ReviewVerdict {
                    verdict: Verdict::RequestChanges,
                    comment: "handle the empty list".into(),
                    items: Vec::new(),
                },
                r#"{"event":"review_verdict","verdict":"request_changes","comment":"handle the empty list"}"#,
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
            (
                AgentEvent::WithdrawThread { thread: "2".into() },
                r#"{"event":"withdraw_thread","thread":"2"}"#,
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
                permission: false,
                detail: None
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
