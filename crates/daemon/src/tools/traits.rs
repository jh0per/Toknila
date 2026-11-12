//! Looking at a repository's folder for what its checks and its remote suggest: the files are read
//! here, the decision of what to suggest is `toknila_core::defaults`.

use std::path::Path;

use toknila_core::defaults::{self, NodeManager, RepoTraits};
use toknila_core::system::Probe;

use super::{TIMEOUT, Tools, git};

fn read(dir: &Path, name: &str) -> Option<String> {
    std::fs::read_to_string(dir.join(name)).ok()
}

fn exists(dir: &Path, name: &str) -> bool {
    dir.join(name).exists()
}

/// Whether `package.json` has a `test` script, and which manager its lock file names (npm when
/// none is there).
fn node_test(dir: &Path) -> Option<NodeManager> {
    let json: serde_json::Value = serde_json::from_str(&read(dir, "package.json")?).ok()?;
    let script = json.get("scripts")?.get("test")?.as_str()?;
    if script.trim().is_empty() {
        return None;
    }
    Some(if exists(dir, "pnpm-lock.yaml") {
        NodeManager::Pnpm
    } else if exists(dir, "yarn.lock") {
        NodeManager::Yarn
    } else {
        NodeManager::Npm
    })
}

/// The folders with a `package-lock.json`: `""` for the root, `apps/<name>` for each one under
/// `apps/`; sorted.
fn npm_lock_dirs(dir: &Path) -> Vec<String> {
    let mut found = Vec::new();
    if exists(dir, "package-lock.json") {
        found.push(String::new());
    }
    if let Ok(entries) = std::fs::read_dir(dir.join("apps")) {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if e.path().join("package-lock.json").is_file() {
                found.push(format!("apps/{name}"));
            }
        }
    }
    found.sort();
    found
}

/// Whether the Makefile has a `test` target (a line `test:` that is not an assignment).
fn make_test(dir: &Path) -> bool {
    let Some(text) = read(dir, "Makefile").or_else(|| read(dir, "makefile")) else {
        return false;
    };
    text.lines().any(|l| {
        let l = l.trim_end();
        l.strip_prefix("test:")
            .is_some_and(|rest| !rest.starts_with('='))
    })
}

/// Whether the folder is a Python project that has tests.
fn python_tests(dir: &Path) -> bool {
    let project = [
        "pyproject.toml",
        "setup.py",
        "setup.cfg",
        "pytest.ini",
        "tox.ini",
        "requirements.txt",
    ]
    .iter()
    .any(|f| exists(dir, f));
    if !project {
        return false;
    }
    if exists(dir, "tests") || exists(dir, "test") {
        return true;
    }
    std::fs::read_dir(dir).is_ok_and(|entries| {
        entries.flatten().any(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            name.starts_with("test_") && name.ends_with(".py")
        })
    })
}

/// Whether an `xtask` package has a `verify` command: a source file of it that names `verify`
/// as a string.
fn xtask_verify(dir: &Path) -> bool {
    let src = dir.join("xtask").join("src");
    let Ok(entries) = std::fs::read_dir(&src) else {
        return false;
    };
    entries
        .flatten()
        .any(|e| std::fs::read_to_string(e.path()).is_ok_and(|t| t.contains("\"verify\"")))
}

fn github_remote(tools: &dyn Tools, dir: &str) -> bool {
    match git(tools, dir, &["remote", "-v"]) {
        Probe::Ran { stdout, .. } => stdout.lines().any(|l| {
            l.split_whitespace()
                .nth(1)
                .is_some_and(defaults::is_github_url)
        }),
        _ => false,
    }
}

fn gh_logged_in(tools: &dyn Tools) -> bool {
    matches!(
        tools.run("gh", &["auth", "status"], TIMEOUT),
        Probe::Ran { code: Some(0), .. }
    )
}

/// What the repository in `dir` shows. `gh` is asked only when the repository has a GitHub remote.
pub(super) fn gather(tools: &dyn Tools, dir: &str) -> RepoTraits {
    let path = Path::new(dir);
    let cargo = exists(path, "Cargo.toml");
    let github_remote = github_remote(tools, dir);
    RepoTraits {
        cargo,
        xtask_verify: cargo && xtask_verify(path),
        node_test: node_test(path),
        make_test: make_test(path),
        npm_lock_dirs: npm_lock_dirs(path),
        python_tests: python_tests(path),
        github_remote,
        gh_logged_in: github_remote && gh_logged_in(tools),
    }
}
