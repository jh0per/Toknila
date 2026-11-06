//! Run logs: `<runs>/<ticket id>/<run id>.log`, the raw bytes of one run's terminal; a reviewer's
//! are in `<runs>/<ticket id>/reviewer/`, so the ticket's own output stays the worker's. The
//! newest one (run ids sort by time) is what a client sees of a ticket whose runner this daemon
//! did not start, after a restart.

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use toknila_core::{AgentRole, RunId, TicketId};

use super::scrollback::Scrollback;

/// The folder of the logs of `role`'s runs of `ticket`.
fn folder(root: &Path, ticket: TicketId, role: AgentRole) -> PathBuf {
    let dir = root.join(ticket.to_string());
    match role {
        AgentRole::Reviewer => dir.join("reviewer"),
        _ => dir,
    }
}

/// Where the output of `run` of `ticket` is kept.
pub(super) fn path(root: &Path, ticket: TicketId, role: AgentRole, run: RunId) -> PathBuf {
    folder(root, ticket, role).join(format!("{run}.log"))
}

/// Creates the log of a new run (and its folder).
pub(super) fn create(
    root: &Path,
    ticket: TicketId,
    role: AgentRole,
    run: RunId,
) -> std::io::Result<File> {
    let path = path(root, ticket, role, run);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    File::create(path)
}

/// The newest run of `ticket` in `role` that has a log.
pub(super) fn latest(root: &Path, ticket: TicketId, role: AgentRole) -> Option<(RunId, PathBuf)> {
    std::fs::read_dir(folder(root, ticket, role))
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|e| {
            let path = e.path();
            let run = path.file_stem()?.to_str()?.parse::<RunId>().ok()?;
            (path.extension()? == "log").then_some((run, path))
        })
        .max_by_key(|(run, _)| *run)
}

/// The last `max_lines` lines of a log, at their offsets in the file.
pub(super) fn load(path: &Path, max_lines: usize) -> std::io::Result<Scrollback> {
    let mut file = File::open(path)?;
    let mut scrollback = Scrollback::new(max_lines);
    let mut buf = vec![0; 64 * 1024];
    loop {
        match file.read(&mut buf)? {
            0 => return Ok(scrollback),
            n => scrollback.push(&buf[..n]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_newest_run_wins_and_foreign_files_are_ignored() {
        let dir = tempfile::tempdir().expect("dir");
        let ticket = TicketId::new();
        let old: RunId = "01JBQ7ZJ0000000000000000R1".parse().expect("id");
        let new: RunId = "01JBQ7ZK0000000000000000R2".parse().expect("id");
        std::fs::write(path_in(&dir, ticket, old), "old\n").expect("old");
        std::fs::write(path_in(&dir, ticket, new), "new\n").expect("new");
        // A reviewer's log never counts as the ticket's own output, and the other way round.
        let rev: RunId = "01JBQ7ZK0000000000000000R3".parse().expect("id");
        create(dir.path(), ticket, AgentRole::Reviewer, rev).expect("reviewer log");
        let (rev_run, _) = latest(dir.path(), ticket, AgentRole::Reviewer).expect("reviewer");
        assert_eq!(rev_run, rev);
        std::fs::write(dir.path().join(ticket.to_string()).join("notes.txt"), "x").expect("x");
        let (run, file) = latest(dir.path(), ticket, AgentRole::Worker).expect("a run");
        assert_eq!(run, new);
        let kept = load(&file, 10).expect("load");
        assert_eq!(kept.from(0), (0, b"new\n".as_slice()));
        assert!(latest(dir.path(), TicketId::new(), AgentRole::Worker).is_none());
    }

    fn path_in(dir: &tempfile::TempDir, ticket: TicketId, run: RunId) -> PathBuf {
        create(dir.path(), ticket, AgentRole::Worker, run).expect("create");
        path(dir.path(), ticket, AgentRole::Worker, run)
    }
}
