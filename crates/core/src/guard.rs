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
