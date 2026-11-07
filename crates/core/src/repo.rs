//! Is this folder usable as a project's repository? Pure: the host gathers [`RepoFacts`] (stat the
//! path, ask git for the branches) and [`judge`] says what is wrong, field by field.

use serde::{Deserialize, Serialize};

/// What the host found at the path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathKind {
    Missing,
    File,
    Dir,
}

/// What git said about the folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitState {
    /// `git` itself is not installed.
    GitMissing,
    /// A folder, but not a work tree.
    NotARepo,
    /// A repository with these local branches.
    Repo { branches: Vec<String> },
}

/// Everything [`judge`] needs to know.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoFacts {
    pub path: PathKind,
    pub git: GitState,
}

/// Which input a problem belongs to, so a client can show it next to that field.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepoField {
    RepoPath,
    WorkBranch,
}

/// What is wrong.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepoProblemCode {
    PathEmpty,
    PathNotAbsolute,
    PathMissing,
    NotADirectory,
    NotAGitRepo,
    GitMissing,
    BranchMissing,
}

/// One thing that stops the folder from being used.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoProblem {
    pub field: RepoField,
    pub code: RepoProblemCode,
    pub message: String,
}

/// The answer to "may this folder be a project's repository, with this base branch?".
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoCheck {
    pub problems: Vec<RepoProblem>,
    /// Local branches of the repository, to offer when the base branch is missing; empty when
    /// the folder is not a repository.
    pub branches: Vec<String>,
    /// The project key the folder suggests; `null` when its name leaves no valid key.
    pub suggested_key: Option<String>,
    /// The project name the folder suggests: its base name.
    pub suggested_name: Option<String>,
}

impl RepoCheck {
    /// The check passed.
    pub fn is_ok(&self) -> bool {
        self.problems.is_empty()
    }
}

fn problem(field: RepoField, code: RepoProblemCode, message: String) -> RepoProblem {
    RepoProblem {
        field,
        code,
        message,
    }
}

fn path_problem(path: &str, facts: &RepoFacts) -> Option<(RepoProblemCode, String)> {
    use RepoProblemCode::*;
    if path.is_empty() {
        return Some((PathEmpty, "Enter the path of the repository.".to_owned()));
    }
    if !std::path::Path::new(path).is_absolute() {
        return Some((PathNotAbsolute, format!("{path} is not an absolute path.")));
    }
    match (&facts.path, &facts.git) {
        (PathKind::Missing, _) => Some((PathMissing, format!("{path} does not exist."))),
        (PathKind::File, _) => Some((NotADirectory, format!("{path} is a file, not a folder."))),
        (PathKind::Dir, GitState::GitMissing) => Some((
            GitMissing,
            "git is not installed, so the folder cannot be checked.".to_owned(),
        )),
        (PathKind::Dir, GitState::NotARepo) => {
            Some((NotAGitRepo, format!("{path} is not a git repository.")))
        }
        (PathKind::Dir, GitState::Repo { .. }) => None,
    }
}

/// Judges the path text first, then what the host found there.
pub fn judge(path: &str, base: &str, facts: &RepoFacts) -> RepoCheck {
    let path = path.trim();
    let suggested_key = crate::suggest::project_key(path);
    let suggested_name = crate::suggest::project_name(path);
    if let Some((code, message)) = path_problem(path, facts) {
        return RepoCheck {
            problems: vec![problem(RepoField::RepoPath, code, message)],
            branches: Vec::new(),
            suggested_key,
            suggested_name,
        };
    }
    let GitState::Repo { branches } = &facts.git else {
        unreachable!("path_problem accepts only a repository");
    };
    let base = base.trim();
    let problems = if branches.iter().any(|b| b == base) {
        Vec::new()
    } else {
        vec![problem(
            RepoField::WorkBranch,
            RepoProblemCode::BranchMissing,
            format!("The repository has no branch {base:?}."),
        )]
    };
    RepoCheck {
        problems,
        branches: branches.clone(),
        suggested_key,
        suggested_name,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(path: PathKind, git: GitState) -> RepoFacts {
        RepoFacts { path, git }
    }

    fn repo(branches: &[&str]) -> RepoFacts {
        facts(
            PathKind::Dir,
            GitState::Repo {
                branches: branches.iter().map(|b| (*b).to_owned()).collect(),
            },
        )
    }

    fn only(c: &RepoCheck) -> (RepoField, RepoProblemCode) {
        assert_eq!(c.problems.len(), 1, "{c:?}");
        (c.problems[0].field, c.problems[0].code)
    }

    #[cfg(unix)]
    const ABS: &str = "/work/toknila";
    #[cfg(windows)]
    const ABS: &str = "C:\\work\\toknila";

    #[test]
    fn a_repo_with_the_base_branch_is_fine() {
        let c = judge(ABS, "develop", &repo(&["main", "develop"]));
        assert!(c.is_ok(), "{c:?}");
        assert_eq!(c.branches, ["main", "develop"]);
    }

    #[test]
    fn empty_and_relative_paths_are_refused_before_looking_at_the_disk() {
        let gone = facts(PathKind::Missing, GitState::NotARepo);
        assert_eq!(
            only(&judge("  ", "develop", &gone)),
            (RepoField::RepoPath, RepoProblemCode::PathEmpty)
        );
        assert_eq!(
            only(&judge("work/toknila", "develop", &gone)),
            (RepoField::RepoPath, RepoProblemCode::PathNotAbsolute)
        );
    }

    #[test]
    fn a_missing_path_a_file_and_a_plain_folder_each_have_their_own_code() {
        let p = |k, g| only(&judge(ABS, "develop", &facts(k, g)));
        assert_eq!(
            p(PathKind::Missing, GitState::NotARepo),
            (RepoField::RepoPath, RepoProblemCode::PathMissing)
        );
        assert_eq!(
            p(PathKind::File, GitState::NotARepo),
            (RepoField::RepoPath, RepoProblemCode::NotADirectory)
        );
        assert_eq!(
            p(PathKind::Dir, GitState::NotARepo),
            (RepoField::RepoPath, RepoProblemCode::NotAGitRepo)
        );
        assert_eq!(
            p(PathKind::Dir, GitState::GitMissing),
            (RepoField::RepoPath, RepoProblemCode::GitMissing)
        );
    }

    #[test]
    fn a_missing_base_branch_is_the_branch_field_and_lists_the_branches_to_pick() {
        let c = judge(ABS, "develop", &repo(&["main", "trunk"]));
        assert_eq!(
            only(&c),
            (RepoField::WorkBranch, RepoProblemCode::BranchMissing)
        );
        assert_eq!(c.branches, ["main", "trunk"]);
        assert!(
            c.problems[0].message.contains("develop"),
            "{}",
            c.problems[0].message
        );
    }

    #[test]
    fn the_check_suggests_a_key_and_a_name_even_for_a_folder_that_is_not_usable() {
        let gone = facts(PathKind::Missing, GitState::NotARepo);
        let c = judge(ABS, "develop", &gone);
        assert_eq!(c.suggested_key.as_deref(), Some("TOKNILA"));
        assert_eq!(c.suggested_name.as_deref(), Some("toknila"));
        let c = judge(ABS, "develop", &repo(&["develop"]));
        assert_eq!(c.suggested_key.as_deref(), Some("TOKNILA"));
    }

    #[test]
    fn a_blank_base_branch_counts_as_missing() {
        assert_eq!(
            only(&judge(ABS, " ", &repo(&["main"]))).1,
            RepoProblemCode::BranchMissing
        );
    }
}
