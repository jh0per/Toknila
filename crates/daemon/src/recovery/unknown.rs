//! Worktree folders under the ships root that nothing names: listed in the log at start, never
//! deleted (a person decides; Settings, Leftover workspaces, shows what git knows).

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use toknila_app::finished_ships;
use toknila_core::{ShipSpec, TicketStatus};

use crate::store::Store;

/// The assistant's reading copy under a project's folder; not a ship, never unknown.
const PLANNER: &str = "_planner";

/// The `<project>/<epic>/<story>` folders under `root` that are in neither `records` (paths ships
/// are recorded at) nor `live` (the `(project, epic, story)` names the running stories' ships
/// have or will have). The `_planner` folder is skipped.
pub fn unknown_folders(
    root: &Path,
    records: &HashSet<PathBuf>,
    live: &HashSet<(String, String, String)>,
) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for project in dirs_of(root) {
        let Some(p) = name_of(&project).filter(|p| p != PLANNER) else {
            continue;
        };
        for chain in dirs_of(&project) {
            let Some(c) = name_of(&chain) else { continue };
            for ship in dirs_of(&chain) {
                let Some(s) = name_of(&ship) else { continue };
                if !records.contains(&ship) && !live.contains(&(p.clone(), c.clone(), s)) {
                    out.push(ship);
                }
            }
        }
    }
    out.sort();
    out
}

fn name_of(path: &Path) -> Option<String> {
    path.file_name().map(|n| n.to_string_lossy().into_owned())
}

fn dirs_of(path: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(path)
        .map(|rd| {
            rd.filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| p.is_dir())
                .collect()
        })
        .unwrap_or_default()
}

/// Logs what start recovery did to the ships: once per retired ship, once per unknown folder.
pub async fn log_ships(store: &Store, root: &Path, retired: &[String]) {
    for key in retired {
        tracing::info!(story = %key, "retiring the ship of a finished story left by an older daemon");
    }
    let root = root.to_owned();
    let found = store.read(move |repo| Ok(look(repo, &root))).await;
    match found {
        Ok(Ok(folders)) => {
            for f in folders {
                tracing::warn!(folder = %f.display(), "a worktree folder that no ship record or running story names; left alone");
            }
        }
        Ok(Err(error)) => tracing::warn!(%error, "could not look for unknown worktree folders"),
        Err(error) => tracing::warn!(%error, "could not look for unknown worktree folders"),
    }
}

fn look(repo: &crate::store::Repo<'_>, root: &Path) -> Result<Vec<PathBuf>, toknila_app::AppError> {
    let records = finished_ships::workspaces(repo, None)?
        .into_iter()
        .map(|w| PathBuf::from(w.ship.path))
        .collect::<HashSet<_>>();
    let mut live = HashSet::new();
    for chain in repo.chains(None)? {
        let Some(project) = repo.project(chain.project_id)? else {
            continue;
        };
        for t in repo.tickets(Some(chain.id))? {
            let done = matches!(
                t.status,
                TicketStatus::Merged | TicketStatus::Released | TicketStatus::Cancelled
            );
            if let (false, Some(spec)) = (done, ShipSpec::for_ticket(&project, &chain, &t)) {
                live.insert((spec.project, spec.chain, spec.slug));
            }
        }
    }
    Ok(unknown_folders(root, &records, &live))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_folder_nothing_names_is_unknown_and_the_planner_copy_is_skipped() {
        let tmp = tempfile::tempdir().expect("tmp");
        let root = tmp.path();
        let mk = |p: &str| {
            let d = root.join(p);
            std::fs::create_dir_all(&d).expect("mkdir");
            d
        };
        let recorded = mk("demo/auth/demo-1-a");
        mk("demo/auth/demo-2-b");
        let stray = mk("demo/auth/demo-3-c");
        mk("demo/_planner");
        let records = HashSet::from([recorded]);
        let live = HashSet::from([("demo".to_owned(), "auth".to_owned(), "demo-2-b".to_owned())]);
        assert_eq!(unknown_folders(root, &records, &live), vec![stray]);
    }
}

#[cfg(test)]
mod log_tests {
    use std::io::Write;
    use std::sync::{Arc, Mutex};

    use super::*;

    /// A log writer that keeps what is written.
    #[derive(Clone)]
    struct Sink(Arc<Mutex<Vec<u8>>>);

    impl Write for Sink {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.lock().expect("lock").extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn start_logs_each_retired_ship_and_each_unknown_folder_once_and_deletes_nothing() {
        let tmp = tempfile::tempdir().expect("tmp");
        let root = tmp.path().join("ships");
        let stray = root.join("demo/auth/demo-9-stray");
        let planner = root.join("demo/_planner");
        for d in [&stray, &planner] {
            std::fs::create_dir_all(d).expect("mkdir");
        }
        std::fs::write(stray.join("keep.txt"), "mine").expect("file");
        let store = Store::open(&tmp.path().join("t.db")).expect("store");
        let buf = Arc::new(Mutex::new(Vec::new()));
        let sink = buf.clone();
        let subscriber = tracing_subscriber::fmt()
            .with_writer(move || Sink(sink.clone()))
            .with_ansi(false)
            .finish();
        let guard = tracing::subscriber::set_default(subscriber);
        let retired = vec!["DEMO-1".to_owned(), "DEMO-2".to_owned()];
        log_ships(&store, &root, &retired).await;
        drop(guard);
        let log = String::from_utf8(buf.lock().expect("lock").clone()).expect("utf8");
        for key in ["DEMO-1", "DEMO-2"] {
            let n = log.lines().filter(|l| l.contains(key)).count();
            assert_eq!(n, 1, "{key} is logged once: {log}");
        }
        let stray_name = stray.display().to_string();
        let n = log.lines().filter(|l| l.contains(&stray_name)).count();
        assert_eq!(n, 1, "the unknown folder is listed once: {log}");
        assert!(
            !log.contains("_planner"),
            "the planner copy is no leftover: {log}"
        );
        assert!(
            stray.join("keep.txt").is_file(),
            "the folder is never deleted"
        );
        assert!(planner.is_dir());
    }
}
