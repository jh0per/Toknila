//! What deleting a project or a chain leaves to do on disk once its records are gone: remove the ships'
//! worktrees from the repository through the shipyard (`git worktree remove --force`, then
//! `git worktree prune`; the `ship/*` branches stay), remove the cards' run files, and, when the
//! person asked, move the repository folder to the system Trash if core allows it. Blocking
//! work, run after the commit and outside any transaction. A step that fails is reported in the
//! answer and never makes the committed deletion look failed.

use std::path::Path;
use std::sync::Arc;

use toknila_api_types::{ChainDeletion, ProjectDeletion};
use toknila_app::removal::Removed;
use toknila_core::PlannerSpec;
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
    // The project's designs and its Figma token go with it.
    crate::designs::Designs::new(&state.home, removed.project.id).delete_all();
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

/// Cleans up after a committed chain deletion: the same steps for its cards, nothing for the
/// repository folder.
pub async fn after_chain_delete(state: &AppState, removed: Removed) -> ChainDeletion {
    let host = Host {
        yard: state.store.shipyard(),
        runners: state.runners.clone(),
        trash: state.trash.clone(),
        home: state.home.clone(),
    };
    let mut out = chain_report(&removed);
    match tokio::task::spawn_blocking(move || {
        let mut done = chain_report(&removed);
        (done.worktrees_removed, done.warnings) = clean_cards(&host, &removed);
        done
    })
    .await
    {
        Ok(done) => done,
        Err(e) => {
            out.warnings.push(format!("the cleanup stopped: {e}"));
            out
        }
    }
}

/// The chain answer before any cleanup.
fn chain_report(removed: &Removed) -> ChainDeletion {
    let chain = removed
        .chains
        .first()
        .cloned()
        .expect("one chain was deleted");
    ChainDeletion {
        display_title: chain.display_title(),
        chain,
        project: removed.project.key.to_string(),
        tickets: removed.tickets.len(),
        worktrees_removed: Vec::new(),
        warnings: Vec::new(),
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
    (out.worktrees_removed, out.warnings) = clean_cards(host, &removed);
    let repo = repo_of(&removed);
    (out.folder, out.folder_error) = match (repo, trash_folder) {
        (None, _) => (FolderFate::None, None),
        (Some(_), false) => (FolderFate::Kept, None),
        (Some(repo), true) => to_trash(host, Path::new(repo)),
    };
    out
}

/// The project's repository folder, if it has one.
fn repo_of(removed: &Removed) -> Option<&str> {
    removed
        .project
        .repo_path
        .as_deref()
        .map(str::trim)
        .filter(|r| !r.is_empty())
}

/// Removes the ships' checkouts from the repository and the cards' run files; returns the
/// checkouts removed and the steps that failed.
fn clean_cards(host: &Host, removed: &Removed) -> (Vec<String>, Vec<String>) {
    let (mut worktrees_removed, mut warnings) = (Vec::new(), Vec::new());
    let repo = repo_of(removed);
    for ship in &removed.ships {
        let removed = match (&host.yard, repo) {
            (Some(yard), Some(repo)) => yard.remove(repo, ship).map_err(|e| e.to_string()),
            _ => Err("no repository to remove it from".to_owned()),
        };
        match removed {
            Ok(()) => worktrees_removed.push(ship.path.clone()),
            Err(e) => warnings.push(format!(
                "the worktree of {} stays at {}: {e}",
                ship.key, ship.path
            )),
        }
    }
    if let (Some(yard), Some(repo)) = (&host.yard, repo)
        && let Some(spec) = PlannerSpec::for_project(&removed.project)
        && let Err(e) = yard.remove_planner_site(repo, &spec.project)
    {
        warnings.push(format!("the planner's checkout stays: {e}"));
    }
    for t in &removed.tickets {
        if let Err(e) = host.runners.forget(t.id) {
            warnings.push(format!("the run files of {} stay: {e}", t.key));
        }
    }
    (worktrees_removed, warnings)
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
