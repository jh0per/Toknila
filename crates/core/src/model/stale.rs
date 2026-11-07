//! A ship made in local merge mode starts from the local base branch. When that branch is behind
//! its upstream the card would start on old code, so the start is refused until a person says
//! what to do. The host reports the facts ([`BaseBehind`]); the decision is made here.

use serde::{Deserialize, Serialize};

/// What the person decided about a base branch that is behind its upstream.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StalePolicy {
    /// Nothing was said: refuse the start.
    #[default]
    Refuse,
    /// "Fast-forward and start": bring the base up to its upstream first.
    FastForward,
    /// "Start anyway": start from the base as it is.
    Allow,
}

/// What the host found: the base branch is behind its upstream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaseBehind {
    pub base: String,
    /// The upstream, as `origin/main`.
    pub upstream: String,
    /// Commits the upstream has that the base lacks; at least one.
    pub behind: u32,
    /// The base can be moved to the upstream without losing anything: it has no commits of its
    /// own, and where it is checked out the checkout has no local changes.
    pub can_fast_forward: bool,
}

/// What to do with a start whose base is behind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BaseCourse {
    /// Make the ship from the base as it is.
    Start,
    /// Move the base to its upstream, then make the ship.
    FastForwardThenStart,
    /// Do not start; the card is blocked with the facts.
    Refuse,
}

impl BaseBehind {
    /// The course the person's `policy` allows. A fast-forward that is not possible is refused,
    /// never turned into "start anyway".
    pub fn course(&self, policy: StalePolicy) -> BaseCourse {
        match policy {
            StalePolicy::Allow => BaseCourse::Start,
            StalePolicy::FastForward if self.can_fast_forward => BaseCourse::FastForwardThenStart,
            StalePolicy::FastForward | StalePolicy::Refuse => BaseCourse::Refuse,
        }
    }
}

impl std::fmt::Display for BaseBehind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&describe(&self.base, &self.upstream, self.behind))
    }
}

/// "base branch main is 3 commits behind origin/main".
pub fn describe(base: &str, upstream: &str, behind: u32) -> String {
    let commits = if behind == 1 { "commit" } else { "commits" };
    format!("base branch {base} is {behind} {commits} behind {upstream}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(can_fast_forward: bool) -> BaseBehind {
        BaseBehind {
            base: "main".into(),
            upstream: "origin/main".into(),
            behind: 133,
            can_fast_forward,
        }
    }

    #[test]
    fn nothing_said_refuses_and_the_words_name_the_facts() {
        assert_eq!(facts(true).course(StalePolicy::Refuse), BaseCourse::Refuse);
        assert_eq!(
            describe("main", "origin/main", 133),
            "base branch main is 133 commits behind origin/main"
        );
        assert_eq!(
            describe("main", "origin/main", 1),
            "base branch main is 1 commit behind origin/main"
        );
    }

    #[test]
    fn fast_forward_needs_it_to_be_possible_and_start_anyway_always_starts() {
        assert_eq!(
            facts(true).course(StalePolicy::FastForward),
            BaseCourse::FastForwardThenStart
        );
        assert_eq!(
            facts(false).course(StalePolicy::FastForward),
            BaseCourse::Refuse
        );
        assert_eq!(facts(false).course(StalePolicy::Allow), BaseCourse::Start);
    }
}
