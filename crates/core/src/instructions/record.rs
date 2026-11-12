//! What the reviewer is shown of the card's record: its threads and the whole hand-in, so that
//! "approved before code" and "the hand-in says" are judged from the record, not from files.

use super::RunBrief;
use crate::{Thread, ThreadKind, ThreadState, Timestamp};

/// One thread of the card, as the reviewer reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BriefThread {
    pub number: u32,
    pub kind: ThreadKind,
    pub title: String,
    pub state: ThreadState,
    /// The version a person approved, when the approval pinned one.
    pub approved_version: Option<u32>,
    /// When a person approved it.
    pub approved_at: Option<Timestamp>,
}

impl BriefThread {
    /// The threads of a card that a reviewer should see: a discussion with the assistant is not a
    /// design and is left out; the rest keep the thread numbers.
    pub fn of(threads: &[Thread]) -> Vec<Self> {
        let mut out: Vec<Self> = threads
            .iter()
            .filter(|t| t.kind != ThreadKind::Discussion)
            .map(|t| Self {
                number: t.number,
                kind: t.kind,
                title: t.title.clone(),
                state: t.state,
                approved_version: t.approved_version,
                approved_at: t.approved_at,
            })
            .collect();
        out.sort_by_key(|t| t.number);
        out
    }

    fn line(&self) -> String {
        let kind = match self.kind {
            ThreadKind::Design => "design",
            ThreadKind::Flow => "flow",
            ThreadKind::Architecture => "architecture",
            ThreadKind::Discussion => "discussion",
            ThreadKind::Other => "other",
        };
        let state = match (self.state, self.approved_version, self.approved_at) {
            (ThreadState::Open, ..) => "open, not approved".to_owned(),
            (ThreadState::Withdrawn, ..) => "withdrawn".to_owned(),
            (ThreadState::Approved, v, at) => {
                let version = v.map_or(String::new(), |v| format!(" version {v}"));
                let when = at.map_or(String::new(), |a| format!(" at {}", a.utc_text()));
                format!("approved{version} by a person{when}")
            }
        };
        format!("- thread {} ({kind}): {}, {state}", self.number, self.title)
    }
}

/// The section with the card's threads; says so when there are none.
pub(super) fn threads_section(brief: &RunBrief<'_>) -> String {
    let mut out = String::from(
        "\n## The card's threads\n\nThe record of the card's design threads. You cannot open \
         them; judge \"the thread shows ... and the owner approved it before code\" from this \
         list (basis `ran`). Times are UTC, to compare with the commit times.\n\n",
    );
    if brief.threads.is_empty() {
        out.push_str("The card has no design threads.\n");
    }
    for t in brief.threads {
        out.push_str(&t.line());
        out.push('\n');
    }
    out
}

/// The section with the whole hand-in report (what changed, the checks, what is left); empty
/// when the worker gave none.
pub(super) fn hand_in_section(brief: &RunBrief<'_>) -> String {
    match brief.hand_in.map(str::trim).filter(|h| !h.is_empty()) {
        Some(text) => {
            let mut out = String::from(
                "\n## The hand-in\n\nThe worker's report, whole. \"The hand-in says\" is judged \
                 from it, with basis `ran`:\n\n",
            );
            for line in text.lines() {
                out.push_str(&format!("> {line}\n").replace("> \n", ">\n"));
            }
            out
        }
        None => String::new(),
    }
}
