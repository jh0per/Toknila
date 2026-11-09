//! The uncommitted-ship guard: whether a ship holds finished work that git can carry. The host
//! looks at the worktree (`git status`, commits ahead of the base) and hands in a [`ShipState`];
//! this module judges it. `finished` and Accept both ask, so no half-done branch is handed in,
//! pushed or merged.

/// What the host saw in a ship's worktree.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ShipState {
    /// Paths with changes that are not committed: tracked changes and untracked files that are
    /// not ignored.
    pub changed: Vec<String>,
    /// Commits on the ship's branch that its starting point does not have.
    pub ahead: u32,
    /// The commit the worktree's HEAD is at; `None` when the host did not say.
    pub head: Option<String>,
}

/// Why a ship's work cannot go on to review or to the base branch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShipProblem {
    /// The worktree has changes that are not committed.
    Uncommitted { files: Vec<String> },
    /// The branch has no commit of its own: there is nothing to hand in.
    Empty,
}

/// Most paths named in a message.
const NAMED: usize = 8;

/// Judges a ship. Uncommitted changes come first: committing them is what fixes both problems.
pub fn judge(state: &ShipState) -> Result<(), ShipProblem> {
    if !state.changed.is_empty() {
        return Err(ShipProblem::Uncommitted {
            files: state.changed.clone(),
        });
    }
    if state.ahead == 0 {
        return Err(ShipProblem::Empty);
    }
    Ok(())
}

/// The Return a hand-in answers, when the ship is still at the commit that was handed in before
/// that Return: nothing was changed, so the hand-in is refused.
pub fn unchanged_since_return<'a>(
    state: &ShipState,
    review: &'a crate::Review,
) -> Option<&'a crate::ReviewRound> {
    let head = state.head.as_deref()?;
    if review.handed_in.as_deref() != Some(head) {
        return None;
    }
    review.open_return()
}

/// What the agent is told when its hand-in after a Return carries no new commit: the Return's
/// text, and that the hand-in must carry new commits.
pub fn unchanged_for_agent(round: &crate::ReviewRound, head: &str) -> String {
    let by = crate::instructions::returned_by(round.reviewer);
    let text = if round.comment.trim().is_empty() {
        "(no text)".to_owned()
    } else {
        round.comment.trim().to_owned()
    };
    format!(
        "Not finished: round {n} was returned by {by} and the branch is still at the commit you \
         handed in before ({short}). A hand-in after a Return must carry new commits. Read the \
         Return, make the change it asks for, commit it, then call `finished` again, naming the \
         new commit.\n\nThe Return:\n{text}",
        n = round.number,
        short = crate::instructions::short_commit(head),
    )
}

/// The paths as a short list: the first few, then how many more.
pub fn list_files(files: &[String]) -> String {
    let mut text = files
        .iter()
        .take(NAMED)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    if files.len() > NAMED {
        text.push_str(&format!(" and {} more", files.len() - NAMED));
    }
    text
}

impl ShipProblem {
    /// What the agent is told when `finished` is refused: what is wrong and what to do.
    pub fn for_agent(&self, start: &str) -> String {
        match self {
            Self::Uncommitted { files } => format!(
                "Not finished: the ship has changes that are not committed ({}). Toknila pushes \
                 the branch, so uncommitted work never reaches the pull request. Commit your work \
                 on this branch (`git add` and `git commit`), then call `finished` again.",
                list_files(files)
            ),
            Self::Empty => format!(
                "Not finished: the branch has no commit of its own on top of {start}. Commit your \
                 work on this branch (`git add` and `git commit`), then call `finished` again."
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(changed: &[&str], ahead: u32) -> ShipState {
        ShipState {
            changed: changed.iter().map(|s| (*s).to_owned()).collect(),
            ahead,
            head: None,
        }
    }

    #[test]
    fn a_clean_ship_with_commits_passes() {
        assert_eq!(judge(&state(&[], 2)), Ok(()));
    }

    #[test]
    fn changes_that_are_not_committed_fail_even_when_commits_exist() {
        assert_eq!(
            judge(&state(&["a.rs", "b.rs"], 3)),
            Err(ShipProblem::Uncommitted {
                files: vec!["a.rs".into(), "b.rs".into()]
            })
        );
    }

    #[test]
    fn nothing_ahead_is_empty_and_dirty_nothing_ahead_is_uncommitted() {
        assert_eq!(judge(&state(&[], 0)), Err(ShipProblem::Empty));
        assert!(matches!(
            judge(&state(&["new.rs"], 0)),
            Err(ShipProblem::Uncommitted { .. })
        ));
    }

    fn returned_review(commit: &str) -> crate::Review {
        let mut r = crate::Review {
            handed_in: Some(commit.to_owned()),
            ..crate::Review::default()
        };
        r.record(
            crate::Actor::Agent,
            crate::Verdict::RequestChanges,
            "Rename the flag",
            crate::Timestamp(1),
        );
        r
    }

    fn at(head: Option<&str>) -> ShipState {
        ShipState {
            ahead: 1,
            head: head.map(str::to_owned),
            ..ShipState::default()
        }
    }

    #[test]
    fn a_hand_in_at_the_returned_commit_is_unchanged_and_a_new_commit_is_not() {
        let review = returned_review("f8afe06aaaa");
        let round = unchanged_since_return(&at(Some("f8afe06aaaa")), &review).expect("same");
        let text = unchanged_for_agent(round, "f8afe06aaaa");
        for want in [
            "round 1",
            "the reviewer",
            "f8afe06",
            "new commits",
            "Rename the flag",
        ] {
            assert!(text.contains(want), "{want}: {text}");
        }
        assert!(unchanged_since_return(&at(Some("b0b0b0b")), &review).is_none());
        assert!(unchanged_since_return(&at(None), &review).is_none());
        // No Return since: the same commit is an ordinary hand-in.
        let mut approved = returned_review("f8afe06aaaa");
        approved.record(
            crate::Actor::Human,
            crate::Verdict::Approve,
            "",
            crate::Timestamp(2),
        );
        assert!(unchanged_since_return(&at(Some("f8afe06aaaa")), &approved).is_none());
        assert!(
            unchanged_since_return(&at(Some("f8afe06aaaa")), &crate::Review::default()).is_none()
        );
    }

    #[test]
    fn the_agent_is_told_to_commit_and_long_lists_are_cut() {
        let many: Vec<String> = (0..12).map(|i| format!("f{i}.rs")).collect();
        let text = ShipProblem::Uncommitted { files: many }.for_agent("origin/main");
        assert!(
            text.contains("f0.rs") && text.contains("and 4 more"),
            "{text}"
        );
        assert!(
            text.contains("git commit") && !text.contains("f9.rs"),
            "{text}"
        );
        let text = ShipProblem::Empty.for_agent("origin/main");
        assert!(
            text.contains("origin/main") && text.contains("git commit"),
            "{text}"
        );
    }
}
