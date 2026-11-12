//! Is this folder usable as a project's repository? Pure: the host gathers [`RepoFacts`] (stat the
//! path, ask git for the branches) and [`judge`] says what is wrong, field by field.

use serde::{Deserialize, Serialize};

use crate::MergeMode;
use crate::defaults::{self, RepoTraits};

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
    /// A repository with these local branches. `head` is the branch HEAD is on (none when
    /// detached) and `remote_default` the branch `origin` names as its default, when known.
    Repo {
        branches: Vec<String>,
        head: Option<String>,
        remote_default: Option<String>,
    },
}

/// No base branch can be told from the repository.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
    "cannot tell the base branch: HEAD is not on a branch and the remote names no default; \
     pass one of the repository's branches ({})",
    branches.join(", ")
)]
pub struct NoDefaultBase {
    pub branches: Vec<String>,
}

/// The base branch of a project nobody named one for: the branch HEAD is on, else the remote's
/// default branch, else an error that names the choices.
pub fn default_base(state: &GitState) -> Result<String, NoDefaultBase> {
    match state {
        GitState::Repo {
            branches,
            head,
            remote_default,
        } => head
            .iter()
            .chain(remote_default)
            .find(|b| !b.trim().is_empty())
            .cloned()
            .ok_or_else(|| NoDefaultBase {
                branches: branches.clone(),
            }),
        _ => Err(NoDefaultBase {
            branches: Vec::new(),
        }),
    }
}

/// Everything [`judge`] needs to know.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoFacts {
    pub path: PathKind,
    pub git: GitState,
    /// What the folder and the machine show about checks and GitHub (see `defaults`).
    pub traits: RepoTraits,
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
    /// A repository whose HEAD names a branch that has no commit yet.
    NoCommits,
}

/// How a problem can be fixed by the daemon (`POST /v1/repos/prepare`).
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepoFix {
    /// Make the folder and its missing parents.
    CreateFolder,
    /// `git init`, then the first commit.
    InitRepository,
    /// An empty commit named "Initial commit".
    FirstCommit,
}

/// One thing that stops the folder from being used.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoProblem {
    pub field: RepoField,
    pub code: RepoProblemCode,
    pub message: String,
    /// What the daemon can do about it; `null` when only the person can.
    #[serde(default)]
    pub fix: Option<RepoFix>,
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
    /// The base branch the repository suggests (where HEAD is, else the remote's default);
    /// `null` when it suggests none.
    pub suggested_branch: Option<String>,
    /// The check command the repository suggests (`cargo xtask verify`, `npm test`, ...); `null`
    /// when it suggests none.
    pub suggested_check_command: Option<String>,
    /// The setup command the repository suggests (`npm ci --prefix apps/web` for a lock file
    /// under `apps/web`); `null` when it suggests none.
    #[serde(default)]
    pub suggested_setup_command: Option<String>,
    /// Where accepted work should land: `github` for a GitHub remote with a working `gh` login,
    /// else `local`; `null` when the folder is not a usable repository.
    pub suggested_merge_mode: Option<MergeMode>,
}

impl RepoCheck {
    /// The check passed.
    pub fn is_ok(&self) -> bool {
        self.problems.is_empty()
    }
}

fn problem(
    field: RepoField,
    code: RepoProblemCode,
    message: String,
    fix: Option<RepoFix>,
) -> RepoProblem {
    RepoProblem {
        field,
        code,
        message,
        fix,
    }
}

type PathProblem = (RepoProblemCode, String, Option<RepoFix>);

fn path_problem(path: &str, facts: &RepoFacts) -> Option<PathProblem> {
    use RepoProblemCode::*;
    if path.is_empty() {
        return Some((
            PathEmpty,
            "Enter the path of the repository.".to_owned(),
            None,
        ));
    }
    if !std::path::Path::new(path).is_absolute() {
        return Some((
            PathNotAbsolute,
            format!("{path} is not an absolute path."),
            None,
        ));
    }
    match (&facts.path, &facts.git) {
        (PathKind::Missing, _) => Some((
            PathMissing,
            format!("{path} does not exist."),
            Some(RepoFix::CreateFolder),
        )),
        (PathKind::File, _) => Some((
            NotADirectory,
            format!("{path} is a file, not a folder."),
            None,
        )),
        (PathKind::Dir, GitState::GitMissing) => Some((
            GitMissing,
            "git is not installed, so the folder cannot be checked.".to_owned(),
            None,
        )),
        (PathKind::Dir, GitState::NotARepo) => Some((
            NotAGitRepo,
            format!("{path} is not a git repository."),
            Some(RepoFix::InitRepository),
        )),
        (PathKind::Dir, GitState::Repo { .. }) => None,
    }
}

/// What is wrong with the base branch; an empty `base` means the repository's own.
fn base_problem(base: &str, branches: &[String], git: &GitState) -> Option<RepoProblem> {
    let field = RepoField::WorkBranch;
    let code = RepoProblemCode::BranchMissing;
    if branches.is_empty() {
        return Some(problem(
            RepoField::RepoPath,
            RepoProblemCode::NoCommits,
            "The repository has no commit yet, so it has no branch to work from.".to_owned(),
            Some(RepoFix::FirstCommit),
        ));
    }
    if base.is_empty() {
        return match default_base(git) {
            Ok(own) if branches.contains(&own) => None,
            Ok(own) => Some(problem(
                field,
                code,
                format!("The repository's own branch {own:?} does not exist; name a branch."),
                None,
            )),
            Err(e) => Some(problem(field, code, format!("Name a branch: {e}."), None)),
        };
    }
    if branches.iter().any(|b| b == base) {
        return None;
    }
    Some(problem(
        field,
        code,
        format!("The repository has no branch {base:?}."),
        None,
    ))
}

/// Judges the path text first, then what the host found there.
pub fn judge(path: &str, base: &str, facts: &RepoFacts) -> RepoCheck {
    let path = path.trim();
    let suggested_key = crate::suggest::project_key(path);
    let suggested_name = crate::suggest::project_name(path);
    if let Some((code, message, fix)) = path_problem(path, facts) {
        return RepoCheck {
            problems: vec![problem(RepoField::RepoPath, code, message, fix)],
            branches: Vec::new(),
            suggested_key,
            suggested_name,
            suggested_branch: None,
            suggested_check_command: None,
            suggested_setup_command: None,
            suggested_merge_mode: None,
        };
    }
    let GitState::Repo { branches, .. } = &facts.git else {
        unreachable!("path_problem accepts only a repository");
    };
    let problems = base_problem(base.trim(), branches, &facts.git)
        .into_iter()
        .collect();
    RepoCheck {
        problems,
        branches: branches.clone(),
        suggested_key,
        suggested_name,
        suggested_branch: default_base(&facts.git).ok(),
        suggested_check_command: defaults::check_command(&facts.traits),
        suggested_setup_command: defaults::setup_command(&facts.traits),
        suggested_merge_mode: Some(defaults::merge_mode(&facts.traits)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(path: PathKind, git: GitState) -> RepoFacts {
        RepoFacts {
            path,
            git,
            traits: RepoTraits::default(),
        }
    }

    fn repo(branches: &[&str]) -> RepoFacts {
        facts(
            PathKind::Dir,
            GitState::Repo {
                branches: branches.iter().map(|b| (*b).to_owned()).collect(),
                head: branches.first().map(|b| (*b).to_owned()),
                remote_default: None,
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

    fn state(branches: &[&str], head: Option<&str>, remote: Option<&str>) -> GitState {
        GitState::Repo {
            branches: branches.iter().map(|b| (*b).to_owned()).collect(),
            head: head.map(str::to_owned),
            remote_default: remote.map(str::to_owned),
        }
    }

    #[test]
    fn the_default_base_is_head_then_the_remote_default_then_an_error_naming_the_choices() {
        let both = state(&["main", "next"], Some("next"), Some("main"));
        assert_eq!(default_base(&both), Ok("next".to_owned()));
        let detached = state(&["main", "next"], None, Some("main"));
        assert_eq!(default_base(&detached), Ok("main".to_owned()));
        let neither = state(&["main", "next"], None, None);
        let err = default_base(&neither).expect_err("none");
        assert_eq!(
            err.to_string(),
            "cannot tell the base branch: HEAD is not on a branch and the remote names no \
             default; pass one of the repository's branches (main, next)"
        );
        let c = judge(ABS, "main", &facts(PathKind::Dir, both));
        assert_eq!(c.suggested_branch.as_deref(), Some("next"));
    }

    #[test]
    fn the_check_suggests_a_check_command_and_a_merge_mode_from_the_traits() {
        let mut f = repo(&["main"]);
        let c = judge(ABS, "main", &f);
        assert_eq!(c.suggested_check_command, None);
        assert_eq!(c.suggested_merge_mode, Some(MergeMode::Local));
        f.traits = RepoTraits {
            cargo: true,
            xtask_verify: true,
            github_remote: true,
            gh_logged_in: true,
            ..RepoTraits::default()
        };
        let c = judge(ABS, "main", &f);
        assert_eq!(
            c.suggested_check_command.as_deref(),
            Some("cargo xtask verify")
        );
        assert_eq!(c.suggested_merge_mode, Some(MergeMode::Github));
        let gone = facts(PathKind::Missing, GitState::NotARepo);
        assert_eq!(judge(ABS, "main", &gone).suggested_merge_mode, None);
    }

    #[test]
    fn a_blank_base_branch_is_the_repositorys_own() {
        let c = judge(ABS, " ", &repo(&["main"]));
        assert!(c.is_ok(), "{c:?}");
        let detached = facts(PathKind::Dir, state(&["main", "next"], None, None));
        let c = judge(ABS, "", &detached);
        assert_eq!(
            only(&c),
            (RepoField::WorkBranch, RepoProblemCode::BranchMissing)
        );
        assert!(c.problems[0].message.contains("Name a branch"), "{c:?}");
        assert_eq!(c.problems[0].fix, None);
    }

    #[test]
    fn a_problem_says_how_it_can_be_fixed() {
        let fix = |f: &RepoFacts| judge(ABS, "main", f).problems[0].fix;
        assert_eq!(
            fix(&facts(PathKind::Missing, GitState::NotARepo)),
            Some(RepoFix::CreateFolder)
        );
        assert_eq!(
            fix(&facts(PathKind::Dir, GitState::NotARepo)),
            Some(RepoFix::InitRepository)
        );
        assert_eq!(fix(&facts(PathKind::File, GitState::NotARepo)), None);
        assert_eq!(
            judge("rel/path", "main", &repo(&["main"])).problems[0].fix,
            None
        );
        let unborn = facts(PathKind::Dir, state(&[], Some("main"), None));
        let c = judge(ABS, "main", &unborn);
        assert_eq!(only(&c), (RepoField::RepoPath, RepoProblemCode::NoCommits));
        assert_eq!(c.problems[0].fix, Some(RepoFix::FirstCommit));
        assert_eq!(c.suggested_branch.as_deref(), Some("main"));
        // An empty base does not hide it.
        assert_eq!(
            judge(ABS, "", &unborn).problems[0].fix,
            Some(RepoFix::FirstCommit)
        );
    }

    #[test]
    fn a_problem_is_pinned_on_the_wire() {
        let c = judge(ABS, "main", &facts(PathKind::Dir, GitState::NotARepo));
        let json = serde_json::to_value(&c.problems[0]).expect("json");
        assert_eq!(json["code"], "not_a_git_repo");
        assert_eq!(json["fix"], "init_repository");
        assert_eq!(json["field"], "repo_path");
        let json = serde_json::to_value(RepoProblemCode::NoCommits).expect("json");
        assert_eq!(json, "no_commits");
    }
}
