//! Questions an agent leaves for a person, apart from the card's own status: what can be
//! answered, how an answer is delivered, and when an agent that shows no sign of life is counted
//! as waiting on its screen. Pure; the daemon detects (a hook fired, a process printed) and
//! delivers (a hook's reply, bytes to the terminal).
//!
//! | Question | What the agent shows | Answers offered |
//! |---|---|---|
//! | `question` | `ask_human` | none here: a comment is the answer |
//! | `permission` | a permission dialog (hook or notification) | `allow`, `deny` |
//! | `screen` | nothing for `startup_secs` after spawn | `enter`, `one`, `two`, `escape`, `text` |

mod screen;

use serde::{Deserialize, Serialize};

use crate::{AgentRole, TicketId, Timestamp};

pub use screen::{SCREEN_LINES, last_lines};

/// Seconds after spawn without a sign of life before an agent counts as waiting on its screen.
pub const DEFAULT_STARTUP_SECS: u64 = 20;

/// The text of the inbox item for an agent that waits on its screen.
pub const SCREEN_TEXT: &str = "The agent waits on its screen";

/// What kind of question is open.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuestionKind {
    /// The agent asked something through `ask_human`.
    Question,
    /// The agent's permission dialog.
    Permission,
    /// The agent shows nothing to Toknila and waits on its screen.
    Screen,
}

/// One way a person answers a question.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnswerKind {
    /// Grant the permission.
    Allow,
    /// Refuse it; the text, if any, tells the agent why.
    Deny,
    /// Free text, typed into the terminal followed by Enter.
    Text,
    /// The Enter key.
    Enter,
    /// The key `1`.
    One,
    /// The key `2`.
    Two,
    /// The Escape key.
    Escape,
}

impl QuestionKind {
    /// The answers a person is offered for this kind of question.
    pub fn answers(self) -> Vec<AnswerKind> {
        match self {
            Self::Question => Vec::new(),
            Self::Permission => vec![AnswerKind::Allow, AnswerKind::Deny],
            Self::Screen => vec![
                AnswerKind::Enter,
                AnswerKind::One,
                AnswerKind::Two,
                AnswerKind::Escape,
                AnswerKind::Text,
            ],
        }
    }

    /// Whether `answer` may be given to this kind of question.
    pub fn accepts(self, answer: AnswerKind) -> bool {
        self.answers().contains(&answer)
    }
}

/// The reply a permission hook waits for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    Allow,
    /// `message` tells the agent why it was refused.
    Deny {
        message: String,
    },
}

/// How an answer reaches the agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Delivery {
    /// The agent's permission hook is waiting for this reply.
    Hook(Decision),
    /// Keys typed into the agent's terminal.
    Keys(Vec<u8>),
}

/// An answer that cannot be delivered.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AnswerError {
    #[error("a text answer needs a text")]
    EmptyText,
    #[error("nothing is asked that {0:?} answers")]
    NotOffered(AnswerKind),
}

/// The message a refused permission carries when the person wrote none.
const DENIED: &str = "The person denied this in the Toknila window.";

/// How `answer` is delivered: to the hook that waits for a decision (`hook_waiting`), else as
/// keys. `Allow` is `1` (the first option of the agent's dialog, Yes) and `Deny` is Escape (the
/// dialog's cancel).
pub fn deliver(
    answer: AnswerKind,
    text: Option<&str>,
    hook_waiting: bool,
) -> Result<Delivery, AnswerError> {
    let text = text.map(str::trim).filter(|t| !t.is_empty());
    Ok(match (answer, hook_waiting) {
        (AnswerKind::Allow, true) => Delivery::Hook(Decision::Allow),
        (AnswerKind::Deny, true) => Delivery::Hook(Decision::Deny {
            message: text.unwrap_or(DENIED).to_owned(),
        }),
        (AnswerKind::Allow | AnswerKind::One, _) => Delivery::Keys(b"1".to_vec()),
        (AnswerKind::Two, _) => Delivery::Keys(b"2".to_vec()),
        (AnswerKind::Deny | AnswerKind::Escape, _) => Delivery::Keys(b"\x1b".to_vec()),
        (AnswerKind::Enter, _) => Delivery::Keys(b"\r".to_vec()),
        (AnswerKind::Text, _) => {
            let text = text.ok_or(AnswerError::EmptyText)?;
            Delivery::Keys(format!("{text}\r").into_bytes())
        }
    })
}

/// What is known about one live run, for the startup rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunFacts {
    pub ticket: TicketId,
    pub role: AgentRole,
    /// When the process was spawned.
    pub spawned: Timestamp,
    /// The first sign of life after the spawn (a hook, an MCP call), if any.
    pub first_activity: Option<Timestamp>,
    /// The profile's `startup_secs`; 0 means the profile does not use the rule.
    pub startup_secs: u64,
    /// The end of the run's output, raw (for the screen lines).
    pub tail: Vec<u8>,
}

impl RunFacts {
    /// Whether the agent waits on its screen at `now`.
    pub fn waits_on_screen(&self, now: Timestamp) -> bool {
        waits_on_screen(self.spawned, self.first_activity, now, self.startup_secs)
    }
}

/// The startup rule: an agent that reported nothing in `startup_secs` after it was spawned waits
/// on its screen (a trust dialog, a login, a first-run question) until its first sign of life.
pub fn waits_on_screen(
    spawned: Timestamp,
    first_activity: Option<Timestamp>,
    now: Timestamp,
    startup_secs: u64,
) -> bool {
    startup_secs > 0
        && first_activity.is_none()
        && now.millis_since(spawned) >= startup_secs.saturating_mul(1000)
}

/// A permission dialog an agent's hook or notification reported, which no card status shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingPermission {
    pub ticket: TicketId,
    pub role: AgentRole,
    /// What is asked, with the tool and its summary.
    pub text: String,
    pub since: Timestamp,
}

#[cfg(test)]
mod tests;
