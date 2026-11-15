//! Shared by the test files that open real ptys (`runner.rs` here, `pty_runner.rs` in the cli):
//! one machine-wide lock, so a loaded computer does not run them all at once. A lock file, not a
//! `static Mutex`, because `cargo nextest` runs each test in its own process and the two files are
//! two binaries. The cli includes this file with `#[path]`; there is no support crate.

use std::fs::OpenOptions;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

/// How long a test waits for its turn before it fails.
const TURN_DEADLINE: Duration = Duration::from_secs(600);
/// A lock file older than this belongs to a test that was killed; it is taken over.
const STALE_AFTER: Duration = Duration::from_secs(300);

fn lock_path() -> PathBuf {
    std::env::temp_dir().join("toknila-pty-tests.lock")
}

/// Holds the pty lock until dropped.
#[derive(Debug)]
pub struct PtyTurn {
    path: PathBuf,
}

impl Drop for PtyTurn {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Waits for the pty lock (a file in the temporary folder, shared by every checkout on this
/// computer) and holds it; fails the test after ten minutes.
pub fn hold() -> PtyTurn {
    hold_for(TURN_DEADLINE).expect("no turn for the pty tests within the deadline")
}

/// [`hold`], giving up after `deadline`.
pub fn hold_for(deadline: Duration) -> Option<PtyTurn> {
    let path = lock_path();
    let until = Instant::now() + deadline;
    loop {
        // `create_new` is atomic: exactly one process creates the file.
        if OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .is_ok()
        {
            return Some(PtyTurn { path });
        }
        let stale = std::fs::metadata(&path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| SystemTime::now().duration_since(t).ok())
            .is_some_and(|age| age > STALE_AFTER);
        if stale {
            let _ = std::fs::remove_file(&path);
            continue;
        }
        if Instant::now() >= until {
            return None;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}
