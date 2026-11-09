//! Looking at a ship's worktree for the uncommitted-ship guard: what `git status` lists and how
//! many commits the branch has beyond where it started.

use toknila_app::{GitError, ShipError};
use toknila_core::ShipLook;
use toknila_core::guard::ShipState;

use super::GitShipyard;

/// The paths in `git status --porcelain=v1 -z` output. An entry is `XY path`; a rename or copy
/// is followed by one more entry with the old path, which is not a change of its own.
pub(super) fn changed_paths(status: &[u8]) -> Vec<String> {
    let mut paths = Vec::new();
    let mut entries = status.split(|b| *b == 0).filter(|e| !e.is_empty());
    while let Some(entry) = entries.next() {
        if entry.len() < 4 {
            continue;
        }
        if matches!(entry[0], b'R' | b'C') || matches!(entry[1], b'R' | b'C') {
            entries.next();
        }
        paths.push(String::from_utf8_lossy(&entry[3..]).into_owned());
    }
    paths
}

impl GitShipyard {
    pub(super) fn look(&self, look: &ShipLook) -> Result<ShipState, ShipError> {
        let dir = look.path.as_str();
        let out = self
            .git
            .output(
                dir,
                &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
            )
            .map_err(ShipError::from)?;
        if !out.status.success() {
            return Err(ShipError::Git(GitError::Failed {
                command: "status".to_owned(),
                detail: String::from_utf8_lossy(&out.stderr).trim().to_owned(),
            }));
        }
        let changed = changed_paths(&out.stdout);
        let range = format!("{}..HEAD", look.start);
        let count = self.git(dir, ["rev-list", "--count", range.as_str()])?;
        let ahead = count.parse().map_err(|_| {
            ShipError::Git(GitError::Failed {
                command: "rev-list --count".to_owned(),
                detail: format!("unexpected answer {count:?}"),
            })
        })?;
        let head = self.git(dir, ["rev-parse", "HEAD"]).ok();
        Ok(ShipState {
            changed,
            ahead,
            head,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_entries_become_paths_and_a_rename_counts_once() {
        let raw = b" M src/a.rs\0?? new file.txt\0R  new.rs\0old.rs\0A  b.rs\0";
        assert_eq!(
            changed_paths(raw),
            ["src/a.rs", "new file.txt", "new.rs", "b.rs"]
        );
        assert!(changed_paths(b"").is_empty());
    }
}
