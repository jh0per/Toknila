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
    /// The check failed once and passed on the second run on the same commit.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub flaky: bool,
    /// The last 30 lines of the first, failed run when `flaky`; empty otherwise.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub flaky_output: String,
}

/// How many lines of the failed first run a flaky pass keeps.
pub const FLAKY_LINES: usize = 30;

impl CheckOutcome {
    /// The first line of the failed first run that says what failed: the first line that names
    /// a failure (`FAIL`, `failed`, `error`, `panicked`), else the last non-empty line.
    pub fn first_failing_line(&self) -> String {
        first_failing_line(&self.flaky_output)
    }

    /// `extra check <paths>` when the round failed on an extra check (its output opens with that
    /// check's headline); `None` when the main command failed or the check passed.
    pub fn failed_extra(&self) -> Option<&str> {
        if self.passed {
            return None;
        }
        let head = self.output.lines().next()?;
        head.starts_with("extra check ")
            .then(|| head.split_once(": `").map_or(head, |(name, _)| name))
    }
}

/// The first line of `output` that names a failure, else its last non-empty line.
pub fn first_failing_line(output: &str) -> String {
    let lines = || output.lines().map(str::trim).filter(|l| !l.is_empty());
    lines()
        .find(|l| {
            let l = l.to_lowercase();
            ["fail", "error", "panicked"].iter().any(|w| l.contains(w))
        })
        .or_else(|| lines().next_back())
        .unwrap_or_default()
        .to_owned()
}

/// The last [`FLAKY_LINES`] lines of `output`.
pub fn last_lines(output: &str) -> String {
    let lines: Vec<&str> = output.trim_end().lines().collect();
    lines[lines.len().saturating_sub(FLAKY_LINES)..].join("\n")
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
            flaky: false,
            flaky_output: String::new(),
        }
    }

    #[test]
    fn the_first_failing_line_and_the_last_thirty_lines() {
        let out = "running 2 tests\ntest a ... ok\ntest b ... FAILED\nsummary";
        assert_eq!(first_failing_line(out), "test b ... FAILED");
        assert_eq!(first_failing_line("one\ntwo\n"), "two");
        let long: String = (1..=50).map(|n| format!("l{n}\n")).collect();
        let kept = last_lines(&long);
        assert_eq!(kept.lines().count(), 30);
        assert!(kept.starts_with("l21") && kept.ends_with("l50"));
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
