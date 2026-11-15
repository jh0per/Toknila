//! The remote of a project's repository, read from git and never assumed: `origin` when the
//! repository has one, else its only remote, else none. The release run and the planner's fetch
//! ask here instead of naming `origin`.

use crate::git::Git;

/// A remote of the repository: its name and the url git has for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Remote {
    pub name: String,
    pub url: String,
}

/// The remote of `repo`: `origin` when it exists, else the single remote, else `None`; also
/// `None` when git cannot tell the url.
pub fn remote_of(git: &Git, repo: &str) -> Option<Remote> {
    let names = git.run(repo, &["remote"]).ok()?;
    let names: Vec<&str> = names
        .lines()
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .collect();
    let name = if names.contains(&"origin") {
        "origin"
    } else if let [only] = names.as_slice() {
        only
    } else {
        return None;
    };
    let url = git.run(repo, &["remote", "get-url", name]).ok()?;
    (!url.is_empty()).then(|| Remote {
        name: name.to_owned(),
        url,
    })
}

/// Whether the branch `base` has an upstream configured.
pub fn has_upstream(git: &Git, repo: &str, base: &str) -> bool {
    let spec = format!("{base}@{{upstream}}");
    git.succeeds(repo, &["rev-parse", "--abbrev-ref", &spec])
        .unwrap_or(false)
}
