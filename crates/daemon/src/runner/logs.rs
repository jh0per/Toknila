//! Run logs: `<runs>/<ticket id>/<run id>.log`, the raw bytes of one run's terminal. The newest
//! one (run ids sort by time) is what a client sees of a ticket whose runner this daemon did
//! not start, after a restart.
//!
//! A log is capped: when the next write would pass the cap the file is renamed to `<run>.log.1`
//! (replacing an earlier one) and a new `<run>.log` starts, so a run keeps its last one to two
//! caps of output. `<run>.base` holds the offset of the first byte of `<run>.log` in the run's
//! output, so offsets stay the run's own after a rotation. A ticket keeps the logs of its
//! newest [`KEEP_RUNS`] runs.

use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use toknila_core::{RunId, TicketId};

use super::scrollback::Scrollback;

/// How many runs of a ticket keep their logs.
pub(super) const KEEP_RUNS: usize = 20;

/// Where the output of `run` of `ticket` is kept.
pub(super) fn path(root: &Path, ticket: TicketId, run: RunId) -> PathBuf {
    root.join(ticket.to_string()).join(format!("{run}.log"))
}

fn rotated(log: &Path) -> PathBuf {
    log.with_extension("log.1")
}

fn base_file(log: &Path) -> PathBuf {
    log.with_extension("base")
}

/// The log being written by a run.
#[derive(Debug)]
pub(super) struct RunLog {
    file: File,
    path: PathBuf,
    cap: u64,
    /// Bytes in the current file.
    written: u64,
    /// Offset in the run's output of the current file's first byte.
    base: u64,
}

impl RunLog {
    /// Creates the log of a new run (and its folder) and drops the logs of the runs the ticket
    /// no longer keeps.
    pub fn create(root: &Path, ticket: TicketId, run: RunId, cap: u64) -> std::io::Result<RunLog> {
        let path = path(root, ticket, run);
        let dir = path.parent().unwrap_or(root);
        std::fs::create_dir_all(dir)?;
        let file = File::create(&path)?;
        prune(dir);
        Ok(RunLog {
            file,
            path,
            cap,
            written: 0,
            base: 0,
        })
    }

    /// Appends output, rotating whenever the file is full; a chunk that does not fit is split, so
    /// a file never holds more than the cap.
    pub fn write(&mut self, mut bytes: &[u8]) -> std::io::Result<()> {
        let cap = self.cap.max(1);
        while !bytes.is_empty() {
            if self.written >= cap {
                self.rotate()?;
            }
            let room = usize::try_from(cap - self.written).unwrap_or(usize::MAX);
            let (now, later) = bytes.split_at(room.min(bytes.len()));
            self.file.write_all(now)?;
            self.file.flush()?;
            self.written += now.len() as u64;
            bytes = later;
        }
        Ok(())
    }

    fn rotate(&mut self) -> std::io::Result<()> {
        let old = rotated(&self.path);
        // Windows does not rename over a file.
        let _ = std::fs::remove_file(&old);
        std::fs::rename(&self.path, &old)?;
        self.base += self.written;
        self.written = 0;
        std::fs::write(base_file(&self.path), self.base.to_string())?;
        self.file = File::create(&self.path)?;
        Ok(())
    }
}

/// Removes the logs (with their rotated file and offset mark) of all but the newest runs.
fn prune(dir: &Path) {
    let mut runs: Vec<(RunId, PathBuf)> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|e| log_run(&e.path()))
        .collect();
    runs.sort_by_key(|(run, _)| *run);
    for (_, log) in runs.iter().take(runs.len().saturating_sub(KEEP_RUNS)) {
        for file in [log.clone(), rotated(log), base_file(log)] {
            let _ = std::fs::remove_file(file);
        }
    }
}

/// The run a `<run>.log` file belongs to.
fn log_run(path: &Path) -> Option<(RunId, PathBuf)> {
    let run = path.file_stem()?.to_str()?.parse::<RunId>().ok()?;
    (path.extension()? == "log").then(|| (run, path.to_owned()))
}

/// The newest run of `ticket` that has a log.
pub(super) fn latest(root: &Path, ticket: TicketId) -> Option<(RunId, PathBuf)> {
    std::fs::read_dir(root.join(ticket.to_string()))
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|e| log_run(&e.path()))
        .max_by_key(|(run, _)| *run)
}

/// The last `max_lines` lines of a log (its rotated half first), at their offsets in the run.
pub(super) fn load(path: &Path, max_lines: usize) -> std::io::Result<Scrollback> {
    let older = rotated(path);
    let older_len = std::fs::metadata(&older).map_or(0, |m| m.len());
    let base: u64 = std::fs::read_to_string(base_file(path))
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0);
    let mut scrollback = Scrollback::starting_at(max_lines, base.saturating_sub(older_len));
    if older_len > 0 {
        feed(&older, &mut scrollback)?;
    }
    feed(path, &mut scrollback)?;
    Ok(scrollback)
}

fn feed(path: &Path, scrollback: &mut Scrollback) -> std::io::Result<()> {
    let mut file = File::open(path)?;
    let mut buf = vec![0; 64 * 1024];
    loop {
        match file.read(&mut buf)? {
            0 => return Ok(()),
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
        std::fs::write(dir.path().join(ticket.to_string()).join("notes.txt"), "x").expect("x");
        let (run, file) = latest(dir.path(), ticket).expect("a run");
        assert_eq!(run, new);
        let kept = load(&file, 10).expect("load");
        assert_eq!(kept.from(0), (0, b"new\n".as_slice()));
        assert!(latest(dir.path(), TicketId::new()).is_none());
    }

    fn run_id(n: u32) -> RunId {
        format!("01JBQ7ZJ{n:018}").parse().expect("id")
    }

    #[test]
    fn a_log_rotates_once_at_its_cap_and_load_restores_the_offsets() {
        let dir = tempfile::tempdir().expect("dir");
        let (ticket, run) = (TicketId::new(), run_id(1));
        let mut log = RunLog::create(dir.path(), ticket, run, 10).expect("create");
        for line in ["aaaa\n", "bbbb\n", "cccc\n", "dddd\n", "eeee\n"] {
            log.write(line.as_bytes()).expect("write");
        }
        let file = path(dir.path(), ticket, run);
        let rotated = rotated(&file);
        assert_eq!(std::fs::read_to_string(&file).expect("log"), "eeee\n");
        assert_eq!(
            std::fs::read_to_string(&rotated).expect("rotated"),
            "cccc\ndddd\n"
        );
        let kept = load(&file, 100).expect("load");
        assert_eq!(kept.end(), 25);
        assert_eq!(kept.from(0), (10, b"cccc\ndddd\neeee\n".as_slice()));
    }

    #[test]
    fn one_chunk_larger_than_the_cap_is_split_across_the_rotation() {
        let dir = tempfile::tempdir().expect("dir");
        let (ticket, run) = (TicketId::new(), run_id(1));
        let mut log = RunLog::create(dir.path(), ticket, run, 4).expect("create");
        log.write(b"abcdefghij").expect("write");
        let file = path(dir.path(), ticket, run);
        assert_eq!(std::fs::read(&file).expect("log"), b"ij");
        assert_eq!(std::fs::read(rotated(&file)).expect("rotated"), b"efgh");
        assert_eq!(
            load(&file, 10).expect("load").from(0),
            (4, b"efghij".as_slice())
        );
    }

    #[test]
    fn a_log_that_never_passes_its_cap_does_not_rotate() {
        let dir = tempfile::tempdir().expect("dir");
        let (ticket, run) = (TicketId::new(), run_id(1));
        let mut log = RunLog::create(dir.path(), ticket, run, 1000).expect("create");
        log.write(b"hello\n").expect("write");
        assert!(!rotated(&path(dir.path(), ticket, run)).exists());
    }

    #[test]
    fn only_the_newest_twenty_runs_of_a_ticket_keep_their_logs() {
        let dir = tempfile::tempdir().expect("dir");
        let ticket = TicketId::new();
        let other = TicketId::new();
        for n in 1..=22 {
            let mut log = RunLog::create(dir.path(), ticket, run_id(n), 4).expect("create");
            log.write(b"aaa\n").expect("write");
            log.write(b"bbb\n").expect("rotate");
        }
        RunLog::create(dir.path(), other, run_id(1), 4).expect("other");
        let mut kept: Vec<String> = std::fs::read_dir(dir.path().join(ticket.to_string()))
            .expect("dir")
            .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".log"))
            .collect();
        kept.sort();
        assert_eq!(kept.len(), 20, "{kept:?}");
        assert_eq!(kept[0], format!("{}.log", run_id(3)));
        assert_eq!(kept[19], format!("{}.log", run_id(22)));
        let all = std::fs::read_dir(dir.path().join(ticket.to_string()))
            .expect("dir")
            .count();
        assert_eq!(
            all,
            20 * 3,
            "rotated logs and offset marks go with their run"
        );
        assert!(latest(dir.path(), other).is_some());
    }

    fn path_in(dir: &tempfile::TempDir, ticket: TicketId, run: RunId) -> PathBuf {
        RunLog::create(dir.path(), ticket, run, u64::MAX).expect("create");
        path(dir.path(), ticket, run)
    }
}
