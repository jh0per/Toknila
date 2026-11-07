//! What deleting a project leaves to do on disk once its records are gone: remove the ships'
//! worktrees from the repository through the shipyard (`git worktree remove --force`, then
//! `git worktree prune`; the `ship/*` branches stay), remove the cards' run files, and, when the
//! person asked, move the repository folder to the system Trash if core allows it. Blocking
//! work, run after the commit and outside any transaction. A step that fails is reported in the
//! answer and never makes the committed deletion look failed.

use std::path::Path;
use std::sync::Arc;

use toknila_api_types::ProjectDeletion;
use toknila_app::removal::Removed;
use toknila_core::removal::{FolderFate, may_trash};
use toknila_core::repo::PathKind;

use crate::plan_files::canonical;
use crate::runner::Runners;
use crate::server::AppState;
use crate::store::Yard;
use crate::trash::Trasher;

/// The host's ports for the cleanup.
struct Host {
    yard: Option<Yard>,
    runners: Arc<Runners>,
    trash: Arc<dyn Trasher>,
    /// The data home: never trashed.
    home: std::path::PathBuf,
}

/// Cleans up after a committed deletion and says what came of it.
pub async fn after_delete(
    state: &AppState,
    removed: Removed,
    trash_folder: bool,
) -> ProjectDeletion {
    let host = Host {
        yard: state.store.shipyard(),
        runners: state.runners.clone(),
        trash: state.trash.clone(),
        home: state.home.clone(),
    };
    let mut fallback = report(&removed);
    match tokio::task::spawn_blocking(move || clean(&host, removed, trash_folder)).await {
        Ok(out) => out,
        Err(e) => {
            fallback.folder = fallback
                .project
                .repo_path
                .as_ref()
                .map_or(FolderFate::None, |_| FolderFate::Kept);
            fallback.warnings.push(format!("the cleanup stopped: {e}"));
            fallback
        }
    }
}

/// The answer before any cleanup: the records are gone, nothing on disk changed yet.
fn report(removed: &Removed) -> ProjectDeletion {
    ProjectDeletion {
        project: removed.project.clone(),
        chains: removed.chains.iter().map(|c| c.name.clone()).collect(),
        tickets: removed.tickets.len(),
        worktrees_removed: Vec::new(),
        folder: FolderFate::None,
        folder_error: None,
        warnings: Vec::new(),
    }
}

fn clean(host: &Host, removed: Removed, trash_folder: bool) -> ProjectDeletion {
    let mut out = report(&removed);
    let repo = removed
        .project
        .repo_path
        .as_deref()
        .map(str::trim)
        .filter(|r| !r.is_empty());
    for ship in &removed.ships {
        let removed = match (&host.yard, repo) {
            (Some(yard), Some(repo)) => yard.remove(repo, ship).map_err(|e| e.to_string()),
            _ => Err("no repository to remove it from".to_owned()),
        };
        match removed {
            Ok(()) => out.worktrees_removed.push(ship.path.clone()),
            Err(e) => out.warnings.push(format!(
                "the worktree of {} stays at {}: {e}",
                ship.key, ship.path
            )),
        }
    }
    for t in &removed.tickets {
        if let Err(e) = host.runners.forget(t.id) {
            out.warnings
                .push(format!("the run files of {} stay: {e}", t.key));
        }
    }
    (out.folder, out.folder_error) = match (repo, trash_folder) {
        (None, _) => (FolderFate::None, None),
        (Some(_), false) => (FolderFate::Kept, None),
        (Some(repo), true) => to_trash(host, Path::new(repo)),
    };
    out
}

/// Moves the repository folder to the Trash when core allows it; otherwise says why it stayed.
fn to_trash(host: &Host, given: &Path) -> (FolderFate, Option<String>) {
    // A relative path is judged as given: resolving it would read it against our own folder.
    let folder = if given.is_absolute() {
        canonical(given)
    } else {
        given.to_owned()
    };
    let kind = match std::fs::metadata(&folder) {
        Ok(m) if m.is_dir() => PathKind::Dir,
        Ok(_) => PathKind::File,
        Err(_) => PathKind::Missing,
    };
    let user_home = dirs::home_dir().map(|h| canonical(&h));
    if let Err(refusal) = may_trash(&folder, &kind, &canonical(&host.home), user_home.as_deref()) {
        return (refusal.fate(), Some(refusal.to_string()));
    }
    match host.trash.trash(&folder) {
        Ok(()) => (FolderFate::Trashed, None),
        Err(e) => (
            FolderFate::Kept,
            Some(format!("{} stayed where it was: {e}", folder.display())),
        ),
    }
}
