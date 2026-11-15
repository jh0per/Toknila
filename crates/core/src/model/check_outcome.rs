//! What the daemon's check command said about a hand-in, kept on the card's review so the reviewer
//! agent can be given it instead of running the gate again.

use serde::{Deserialize, Serialize};

use super::Timestamp;

/// The result of the project's check command on one hand-in.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckOutcome {
    /// The command that ran; empty when the checker never said which.
    #[serde(default)]
    pub command: String,
    pub passed: bool,
    /// The command and the end of its output, as the worker reads it.
    pub output: String,
    /// When the check finished.
    pub at: Timestamp,
    /// The commit of main the ship was synced to when the check ran; `None` when the host could
    /// not say (a check of an older version).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<String>,
    /// The ship's HEAD the check ran on; `None` when the host could not say.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head: Option<String>,
}

/// What Accept knows about the last check when the merge runs: the main commit it saw. A merge
/// carries it only when the project has a check command; without one nothing is checked again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recheck {
    /// The main commit of the last passed check; `None` when no check recorded one.
    pub base: Option<String>,
}

impl Recheck {
    /// What the last check of `last` saw, when it passed.
    pub fn of(last: Option<&CheckOutcome>) -> Self {
        Self {
            base: last.filter(|c| c.passed).and_then(|c| c.base.clone()),
        }
    }

    /// Whether the card must be checked again because main is at `main` now: the last passed
    /// check saw another commit, or none.
    pub fn due(&self, main: &str) -> bool {
        self.base.as_deref() != Some(main)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outcome(passed: bool, base: Option<&str>) -> CheckOutcome {
        CheckOutcome {
            command: "c".to_owned(),
            passed,
            output: String::new(),
            at: Timestamp(1),
            base: base.map(str::to_owned),
            head: None,
        }
    }

    #[test]
    fn a_check_is_due_when_main_moved_or_the_check_saw_no_main() {
        let saw_a = Recheck::of(Some(&outcome(true, Some("aaa"))));
        assert!(!saw_a.due("aaa"));
        assert!(saw_a.due("bbb"));
        assert!(Recheck::of(Some(&outcome(true, None))).due("aaa"));
        assert!(Recheck::of(Some(&outcome(false, Some("aaa")))).due("aaa"));
        assert!(Recheck::of(None).due("aaa"));
    }
}
