//! Helpers for tests that run the real `git` on temporary repositories.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The canonical form of `p` in the spelling git accepts everywhere: on Windows
/// `canonicalize` adds the `\\?\` prefix, which git refuses ("could not create leading
/// directories"), so it goes.
pub(crate) fn canonical(p: &Path) -> PathBuf {
    let c = p.canonicalize().expect("canonical");
    #[cfg(windows)]
    {
        let s = c.to_string_lossy();
        if let Some(rest) = s.strip_prefix(r"\\?\") {
            return PathBuf::from(rest);
        }
    }
    c
}

/// Runs `git -C dir args` with a fixed identity and returns its trimmed stdout; panics on failure.
pub(crate) fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .output()
        .expect("git runs");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

/// Makes `repo` (and its parents) a repository whose only branch is `branch`, with one commit that
/// adds `README.md`.
pub(crate) fn init_repo(repo: &Path, branch: &str) {
    std::fs::create_dir_all(repo).expect("mkdir");
    git(repo, &["init", "--quiet"]);
    git(
        repo,
        &["symbolic-ref", "HEAD", &format!("refs/heads/{branch}")],
    );
    commit_file(repo, "README.md", "hi\n", "init");
}

/// Writes `name` in `dir` and commits it with `message`.
pub(crate) fn commit_file(dir: &Path, name: &str, content: &str, message: &str) {
    std::fs::write(dir.join(name), content).expect("write");
    git(dir, &["add", name]);
    git(dir, &["commit", "--quiet", "-m", message]);
}
