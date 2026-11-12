//! What to suggest for a new project from the facts of its repository: the check command and
//! where accepted work lands. Pure: the host looks at the folder and gathers [`RepoTraits`]; the
//! decision is made here, so every client (the web form, `tok project new`) suggests the same.

use crate::MergeMode;

/// How a JavaScript project is driven, told by its lock file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeManager {
    Npm,
    Pnpm,
    Yarn,
}

impl NodeManager {
    /// The test command of the manager.
    fn test(self) -> &'static str {
        match self {
            Self::Npm => "npm test",
            Self::Pnpm => "pnpm test",
            Self::Yarn => "yarn test",
        }
    }
}

/// What the host found in the repository's folder and on the machine.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RepoTraits {
    /// A `Cargo.toml` at the top.
    pub cargo: bool,
    /// An `xtask` package that has a `verify` command.
    pub xtask_verify: bool,
    /// A `package.json` with a `test` script, and the manager its lock file names.
    pub node_test: Option<NodeManager>,
    /// A `Makefile` with a `test` target.
    pub make_test: bool,
    /// The folders that hold a `package-lock.json`, relative to the repository: `""` for its
    /// root, `apps/web` for one under `apps/`. Sorted.
    pub npm_lock_dirs: Vec<String>,
    /// A Python project (`pyproject.toml`, `setup.py`, `setup.cfg`, `pytest.ini`, `tox.ini` or
    /// `requirements.txt`) that has tests.
    pub python_tests: bool,
    /// A remote of the repository points at GitHub.
    pub github_remote: bool,
    /// `gh auth status` succeeded.
    pub gh_logged_in: bool,
}

/// The check command the repository suggests, or `None` when it suggests none. The first that
/// applies wins: a verify command of the repository's own, then the toolchain's test command.
pub fn check_command(t: &RepoTraits) -> Option<String> {
    let command = if t.cargo && t.xtask_verify {
        "cargo xtask verify"
    } else if t.cargo {
        "cargo test"
    } else if let Some(manager) = t.node_test {
        manager.test()
    } else if t.make_test {
        "make test"
    } else if t.python_tests {
        "pytest"
    } else {
        return None;
    };
    Some(command.to_owned())
}

/// The setup command the repository suggests, or `None`: `npm ci` for a lock file at the root,
/// `npm ci --prefix apps/web` for one under `apps/`; several are joined with `&&`.
pub fn setup_command(t: &RepoTraits) -> Option<String> {
    let steps: Vec<String> = t
        .npm_lock_dirs
        .iter()
        .map(|dir| match dir.as_str() {
            "" => "npm ci".to_owned(),
            dir => format!("npm ci --prefix {dir}"),
        })
        .collect();
    (!steps.is_empty()).then(|| steps.join(" && "))
}

/// Where accepted work lands by default: a pull request when there is a GitHub remote and `gh`
/// can act on it, else the local base branch.
pub fn merge_mode(t: &RepoTraits) -> MergeMode {
    if t.github_remote && t.gh_logged_in {
        MergeMode::Github
    } else {
        MergeMode::Local
    }
}

/// Whether a remote URL points at GitHub (`https://github.com/o/r`, `git@github.com:o/r.git`,
/// `ssh://git@github.com/o/r`).
pub fn is_github_url(url: &str) -> bool {
    let rest = url.trim();
    let rest = rest.split_once("://").map_or(rest, |(_, r)| r);
    let rest = rest.rsplit_once('@').map_or(rest, |(_, r)| r);
    let host = rest.split([':', '/']).next().unwrap_or("");
    host.eq_ignore_ascii_case("github.com") || host.eq_ignore_ascii_case("www.github.com")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t() -> RepoTraits {
        RepoTraits::default()
    }

    #[test]
    fn a_repository_that_shows_nothing_suggests_no_check() {
        assert_eq!(check_command(&t()), None);
    }

    #[test]
    fn a_lock_file_at_the_root_or_under_apps_suggests_npm_ci() {
        let dirs = |d: &[&str]| RepoTraits {
            npm_lock_dirs: d.iter().map(|s| (*s).to_owned()).collect(),
            ..t()
        };
        assert_eq!(setup_command(&t()), None);
        assert_eq!(setup_command(&dirs(&[""])).as_deref(), Some("npm ci"));
        assert_eq!(
            setup_command(&dirs(&["apps/web"])).as_deref(),
            Some("npm ci --prefix apps/web")
        );
        assert_eq!(
            setup_command(&dirs(&["", "apps/web"])).as_deref(),
            Some("npm ci && npm ci --prefix apps/web")
        );
    }

    #[test]
    fn xtask_verify_beats_cargo_test_and_needs_a_cargo_workspace() {
        let both = RepoTraits {
            cargo: true,
            xtask_verify: true,
            ..t()
        };
        assert_eq!(check_command(&both).as_deref(), Some("cargo xtask verify"));
        let plain = RepoTraits { cargo: true, ..t() };
        assert_eq!(check_command(&plain).as_deref(), Some("cargo test"));
        let orphan = RepoTraits {
            xtask_verify: true,
            ..t()
        };
        assert_eq!(check_command(&orphan), None);
    }

    #[test]
    fn a_node_project_runs_its_test_script_with_the_manager_of_its_lock_file() {
        for (m, text) in [
            (NodeManager::Npm, "npm test"),
            (NodeManager::Pnpm, "pnpm test"),
            (NodeManager::Yarn, "yarn test"),
        ] {
            let node = RepoTraits {
                node_test: Some(m),
                ..t()
            };
            assert_eq!(check_command(&node).as_deref(), Some(text));
        }
    }

    #[test]
    fn make_and_pytest_come_after_the_toolchains_in_that_order() {
        let both = RepoTraits {
            make_test: true,
            python_tests: true,
            ..t()
        };
        assert_eq!(check_command(&both).as_deref(), Some("make test"));
        let py = RepoTraits {
            python_tests: true,
            ..t()
        };
        assert_eq!(check_command(&py).as_deref(), Some("pytest"));
        let node_and_make = RepoTraits {
            node_test: Some(NodeManager::Npm),
            make_test: true,
            ..t()
        };
        assert_eq!(check_command(&node_and_make).as_deref(), Some("npm test"));
    }

    #[test]
    fn pull_requests_need_a_github_remote_and_a_login() {
        let both = RepoTraits {
            github_remote: true,
            gh_logged_in: true,
            ..t()
        };
        assert_eq!(merge_mode(&both), MergeMode::Github);
        let no_login = RepoTraits {
            github_remote: true,
            ..t()
        };
        assert_eq!(merge_mode(&no_login), MergeMode::Local);
        let no_remote = RepoTraits {
            gh_logged_in: true,
            ..t()
        };
        assert_eq!(merge_mode(&no_remote), MergeMode::Local);
    }

    #[test]
    fn github_urls_are_told_from_others() {
        for yes in [
            "https://github.com/jh0per/Toknila.git",
            "git@github.com:jh0per/Toknila.git",
            "ssh://git@github.com/jh0per/Toknila",
            "https://user:token@github.com/o/r",
        ] {
            assert!(is_github_url(yes), "{yes}");
        }
        for no in [
            "https://gitlab.com/o/r",
            "git@example.com:github.com/r.git",
            "https://notgithub.com/o/r",
            "/srv/git/repo.git",
            "",
        ] {
            assert!(!is_github_url(no), "{no}");
        }
    }
}
