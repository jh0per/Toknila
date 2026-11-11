//! The hand-in a `review` item carries: the agent's last report in the template (gist, what
//! changed, the checks, what is left) and the Since the Return block, so a row shows it without
//! loading the card.

use serde::{Deserialize, Serialize};

use crate::{Actor, Comment, CommentKind, SinceReturn, Ticket};

/// How a check ended, as the agent wrote it in its hand-in.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HandInStatus {
    #[serde(rename = "PASS")]
    Pass,
    #[serde(rename = "FAIL")]
    Fail,
    #[serde(rename = "NOT_RUN")]
    NotRun,
    #[serde(rename = "BLOCKED")]
    Blocked,
}

impl HandInStatus {
    /// The word the agent wrote.
    pub fn word(self) -> &'static str {
        match self {
            Self::Pass => "PASS",
            Self::Fail => "FAIL",
            Self::NotRun => "NOT_RUN",
            Self::Blocked => "BLOCKED",
        }
    }
}

/// One line of the checks block.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InboxCheck {
    /// The status; absent for a line that is no check (it stays text).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<HandInStatus>,
    /// The evidence, or the whole line when it has no status.
    pub text: String,
}

/// The hand-in of a card in review.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InboxHandIn {
    /// The one-line gist of what changed.
    pub gist: String,
    /// The lines under "What changed", when the agent wrote any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub what: Option<String>,
    /// The checks, in the order the agent wrote them.
    pub checks: Vec<InboxCheck>,
    /// What is left or not covered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub left: Option<String>,
    /// What changed since the Return this hand-in answers, when it answers one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub since_return: Option<SinceReturn>,
}

/// One line of a checks block as a status and its evidence.
pub fn parse_check(line: &str) -> InboxCheck {
    let rest = line.trim().trim_start_matches(['-', '*']).trim_start();
    for status in [
        HandInStatus::Pass,
        HandInStatus::Fail,
        HandInStatus::NotRun,
        HandInStatus::Blocked,
    ] {
        let Some(after) = rest.strip_prefix(status.word()) else {
            continue;
        };
        if after
            .chars()
            .next()
            .is_some_and(|c| c.is_alphanumeric() || c == '_')
        {
            continue;
        }
        return InboxCheck {
            status: Some(status),
            text: after.trim_start_matches([':', ' ', '\t']).to_owned(),
        };
    }
    InboxCheck {
        status: None,
        text: line.trim().to_owned(),
    }
}

/// The hand-in of `card` from its comments: the newest agent report that has checks (the daemon
/// keeps the checks block in `how_to_verify`); `None` when the card has none.
pub fn of_card(card: &Ticket, comments: &[Comment]) -> Option<InboxHandIn> {
    let c = comments.iter().rev().find(|c| {
        c.ticket_id == card.id
            && c.kind == CommentKind::Report
            && c.author == Actor::Agent
            && c.body
                .how_to_verify
                .as_deref()
                .is_some_and(|v| !v.trim().is_empty())
    })?;
    let checks = c
        .body
        .how_to_verify
        .iter()
        .flat_map(|v| v.lines())
        .filter(|l| !l.trim().is_empty())
        .map(parse_check)
        .collect();
    Some(InboxHandIn {
        gist: c.body.gist.clone(),
        what: c.body.what.clone(),
        checks,
        left: c.body.consequences.clone(),
        since_return: card.review.since_return.clone(),
    })
}

impl InboxHandIn {
    /// The checks as one line: `PASS 2, FAIL 1`; empty when there are none.
    pub fn checks_line(&self) -> String {
        let mut parts = Vec::new();
        for s in [
            HandInStatus::Pass,
            HandInStatus::Fail,
            HandInStatus::NotRun,
            HandInStatus::Blocked,
        ] {
            let n = self.checks.iter().filter(|c| c.status == Some(s)).count();
            if n > 0 {
                parts.push(format!("{} {n}", s.word()));
            }
        }
        parts.join(", ")
    }
}
