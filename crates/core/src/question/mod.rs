//! Questions an agent leaves for a person, apart from the card's own status: what can be
//! answered, how an answer is delivered, and when an agent that shows no sign of life is counted
//! as waiting on its screen. Pure; the daemon detects (a hook fired, a process printed) and
//! delivers (a hook's reply, bytes to the terminal).
//!
//! | Question | What the agent shows | Answers offered |
//! |---|---|---|
//! | `question` | `ask_human` | none here: a comment is the answer |
//! | `permission` | a permission dialog (hook or notification) | `allow`, `deny` |
//! | `screen` | nothing for `startup_secs` after spawn | `enter`, `one`, `two`, `up`, `down`, `escape`, `text` (which may carry named keys like `{down}`) |

mod choices;
mod keys;
mod screen;
mod shell_edit;
mod trust;

use serde::{Deserialize, Serialize};

use crate::{AgentRole, TicketId, TicketStatus, Timestamp};

pub use choices::{ChoiceError, MAX_CHOICES, MIN_CHOICES, check_choices, choice_text};
pub use keys::{expand_keys, has_named_keys};
pub use screen::{SCREEN_LINES, last_lines, screen_hash};
pub use shell_edit::{SHELL_EDIT_DENIAL, shell_edit_denial};
pub use trust::{TRUST_NO, TRUST_YES, is_trust_dialog, trust_keys};

/// Seconds after spawn without a sign of life before an agent counts as waiting on its screen.
pub const DEFAULT_STARTUP_SECS: u64 = 20;

/// The text of the inbox item for an agent that waits on its screen.
pub const SCREEN_TEXT: &str = "The agent waits on its screen";

/// What the inbox row and `tok ls` say of a card that waits on its screen.
pub const SCREEN_WAITING: &str = "Claude Code asks on its screen";

/// The journal's words for a screen question: what its first line says.
pub fn screen_words(lines: &[String]) -> String {
    lines.first().map_or_else(
        || "the agent's screen asks something".to_owned(),
        |first| format!("the agent's screen asks: {}", first.trim()),
    )
}

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
    /// The Up arrow key.
    Up,
    /// The Down arrow key.
    Down,
    /// Choose "Yes, I trust this folder" on Claude Code's trust dialog.
    Yes,
    /// Choose "No, exit" on the trust dialog.
    No,
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
                AnswerKind::Up,
                AnswerKind::Down,
                AnswerKind::Escape,
                AnswerKind::Text,
            ],
        }
    }

    /// Whether `answer` may be given to this kind of question. The two trust answers are for a
    /// screen only; the keys they type depend on the screen ([`trust_keys`]).
    pub fn accepts(self, answer: AnswerKind) -> bool {
        self.answers().contains(&answer)
            || (self == Self::Screen && matches!(answer, AnswerKind::Yes | AnswerKind::No))
            // An `ask_human` question is answered with free text (or one of its choices).
            || (self == Self::Question && answer == AnswerKind::Text)
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

/// What the worker's permission hook is told when it asks after the hand-in.
pub const HANDED_IN_DENIAL: &str = "This story is handed in and waits for review. Nothing more should run, so this is refused automatically; stop here and do not verify again, unless the story is returned to you.";

/// The answer a permission request gets without asking a person: a worker whose card is in
/// Review has handed in (`finished` was accepted), so anything it still wants to run, such as a
/// second verification, is refused with a reason. A returned card is Running again and asks as
/// usual. Anything else is for a person to decide.
pub fn auto_decision(role: AgentRole, status: TicketStatus) -> Option<Decision> {
    (role == AgentRole::Worker && status == TicketStatus::Review).then(|| Decision::Deny {
        message: HANDED_IN_DENIAL.to_owned(),
    })
}

/// Every decision that needs no person, for one tool call: a worker that handed in is refused
/// ([`auto_decision`]), and a worker's shell edit is refused ([`shell_edit_denial`]; `scratch` are
/// the folders it may write). The PermissionRequest path and the PreToolUse path both ask this, so
/// the refusal is the same in every permission mode. `None` leaves the call to the agent or a person.
pub fn automatic_decision(
    role: AgentRole,
    status: TicketStatus,
    tool: Option<&str>,
    scratch: &[String],
) -> Option<Decision> {
    auto_decision(role, status).or_else(|| {
        (role == AgentRole::Worker)
            .then(|| tool.and_then(|tool| shell_edit_denial(tool, scratch)))
            .flatten()
    })
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
        // Only a screen that is the trust dialog says which keys these are.
        (AnswerKind::Yes | AnswerKind::No, _) => return Err(AnswerError::NotOffered(answer)),
        (AnswerKind::Allow, true) => Delivery::Hook(Decision::Allow),
        (AnswerKind::Deny, true) => Delivery::Hook(Decision::Deny {
            message: text.unwrap_or(DENIED).to_owned(),
        }),
        (AnswerKind::Allow | AnswerKind::One, _) => Delivery::Keys(b"1".to_vec()),
        (AnswerKind::Two, _) => Delivery::Keys(b"2".to_vec()),
        (AnswerKind::Deny | AnswerKind::Escape, _) => Delivery::Keys(b"\x1b".to_vec()),
        (AnswerKind::Enter, _) => Delivery::Keys(b"\r".to_vec()),
        (AnswerKind::Up, _) => Delivery::Keys(b"\x1b[A".to_vec()),
        (AnswerKind::Down, _) => Delivery::Keys(b"\x1b[B".to_vec()),
        (AnswerKind::Text, _) => {
            let text = text.ok_or(AnswerError::EmptyText)?;
            // Named keys (`{down}{enter}`) are the whole answer: no Enter is added to them.
            if has_named_keys(text) {
                Delivery::Keys(expand_keys(text))
            } else {
                Delivery::Keys(format!("{text}\r").into_bytes())
            }
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
    /// The last time a person answered this run's screen with keys, and the screen then.
    pub answered: Option<ScreenAnswer>,
}

/// A person's answer by keys to a run's screen: when, and a hash of the screen it was given to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScreenAnswer {
    pub at: Timestamp,
    pub screen: u64,
}

/// How long after an answer the same unchanged screen is not raised again, in seconds.
pub const ANSWER_GRACE_SECS: u64 = 60;

impl RunFacts {
    /// Whether the agent waits on its screen at `now`.
    pub fn waits_on_screen(&self, now: Timestamp) -> bool {
        let Some(answer) = self.answered else {
            return waits_on_screen(self.spawned, self.first_activity, now, self.startup_secs);
        };
        // After an answer, a screen that is still the one it was given to is not raised again
        // for a longer grace period (the keys changed nothing; do not nag). A screen that has
        // changed is something new to look at, so the item is there at once: a person who
        // answers with several keys (a Down, then Enter) must not wait between them.
        let unchanged = screen_hash(&self.tail) == answer.screen;
        let grace = ANSWER_GRACE_SECS.saturating_mul(1000);
        if unchanged && now.millis_since(answer.at) < grace {
            return false;
        }
        waits_on_screen(self.spawned, self.first_activity, now, self.startup_secs)
    }

    /// When the item for this run began: the startup time after the spawn, or the last answer.
    pub fn waiting_since(&self) -> Timestamp {
        self.answered.map_or_else(
            || {
                let wait = i64::try_from(self.startup_secs.saturating_mul(1000)).unwrap_or(0);
                Timestamp(self.spawned.0.saturating_add(wait))
            },
            |a| a.at,
        )
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
    /// The whole command when `text` carries only its summary.
    pub detail: Option<String>,
    pub since: Timestamp,
}

#[cfg(test)]
mod tests;
