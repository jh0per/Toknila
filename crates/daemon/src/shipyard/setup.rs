//! The mark that the project's setup command ran in a ship. It lives in the worktree's own git
//! directory (`<repo>/.git/worktrees/<name>`), so it never shows as a change in the ship and goes
//! away with the worktree: a recreated ship starts without it and runs the setup again.

use std::path::PathBuf;

use super::GitShipyard;

/// The mark's file name.
const MARK: &str = "toknila-setup-ran";

impl GitShipyard {
    fn setup_mark(&self, dir: &str) -> Option<PathBuf> {
        let git_dir = self
            .git
            .run(dir, &["rev-parse", "--absolute-git-dir"])
            .ok()?;
        Some(PathBuf::from(git_dir).join(MARK))
    }

    pub(super) fn has_setup_mark(&self, dir: &str) -> bool {
        self.setup_mark(dir).is_some_and(|m| m.exists())
    }

    pub(super) fn set_setup_mark(&self, dir: &str) {
        if let Some(mark) = self.setup_mark(dir)
            && let Err(e) = std::fs::write(&mark, b"")
        {
            tracing::warn!(mark = %mark.display(), "cannot mark the setup as run: {e}");
        }
    }
}
