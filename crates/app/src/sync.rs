//! Bringing a ship up to its base: when the base branch moved since the ship started, the base is
//! merged into the ship (a merge commit, no rebase) before the hand-in is checked and before
//! Accept merges. A [`crate::Merger`] does it on the host; a conflict is not an error: it comes
//! back as text the worker can act on, with the conflicting files and any migration number that
//! both branches took, named.

use toknila_core::MergeSpec;
use toknila_core::migrations::{self, Clash};

/// What bringing a ship up to its base needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncSpec {
    /// The repository the ship belongs to.
    pub repo: String,
    /// What the ship is brought up to: the base branch (or the remote's copy of it).
    pub base: String,
    /// The ship's worktree, where the merge commit is made.
    pub path: String,
    /// The merge commit's message.
    pub message: String,
}

impl SyncSpec {
    /// The sync that goes with the merge of `spec`.
    pub fn of(spec: &MergeSpec) -> Self {
        Self {
            repo: spec.repo.clone(),
            base: spec.look.start.clone(),
            path: spec.look.path.clone(),
            message: format!("Merge {} into {}", spec.look.start, spec.branch),
        }
    }
}

/// How bringing a ship up ended when nothing went wrong on the host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncOutcome {
    /// The ship already contains the base: nothing was done.
    UpToDate,
    /// The base was merged into the ship; the ship is at `commit`.
    Merged { commit: String },
    /// The merge cannot be made as it is; the ship is untouched.
    Conflict {
        /// The files both branches changed in conflicting ways.
        files: Vec<String>,
        /// Migration numbers both branches took.
        migrations: Vec<Clash>,
    },
}

/// The outcome that records a base-sync merge commit the daemon made, if `synced` made one.
pub(crate) fn recorded(
    ticket: toknila_core::TicketId,
    synced: &Result<SyncOutcome, crate::MergeError>,
) -> Option<crate::external::ExternalOutcome> {
    match synced {
        Ok(SyncOutcome::Merged { commit }) => Some(crate::external::ExternalOutcome::Synced {
            ticket,
            commit: commit.clone(),
        }),
        _ => None,
    }
}

/// What the worker reads about a conflict: the files, the migrations, and what to do.
pub fn conflict_text(base: &str, files: &[String], clashes: &[Clash]) -> String {
    let mut text = format!(
        "{base} moved since this ship started, and merging it into the ship does not go through."
    );
    if !files.is_empty() {
        text.push_str(&format!("\nConflicting files: {}.", files.join(", ")));
    }
    for clash in clashes {
        text.push('\n');
        text.push_str(&migrations::describe(clash, base));
        text.push('.');
    }
    text.push_str(&format!(
        "\nRun `git merge {base}` in the ship, resolve it, commit, then finish again."
    ));
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_text_names_files_and_migrations() {
        let clash = Clash {
            number: 16,
            ship: "mine".to_owned(),
            base: "theirs".to_owned(),
        };
        let text = conflict_text("main", &["a.rs".to_owned(), "b.rs".to_owned()], &[clash]);
        assert!(text.contains("Conflicting files: a.rs, b.rs."), "{text}");
        assert!(text.contains("migration 16 is taken twice"), "{text}");
        assert!(text.contains("\"theirs\""), "{text}");
        assert!(text.contains("git merge main"), "{text}");
    }
}
