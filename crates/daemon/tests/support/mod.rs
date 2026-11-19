//! Shared by the test files that open real ptys (`runner.rs` here, `pty_runner.rs` in the cli):
//! one lock per test run, so one run does not open every pty at once. A lock file, not a
//! `static Mutex`, because `cargo nextest` runs each test in its own process and the two files are
//! two binaries. The cli includes this file with `#[path]`; there is no support crate.
//!
//! The lock is scoped to the run (`TOKNILA_TEST_ROOT`, which `cargo xtask verify` sets) or, under
//! a plain `cargo test`, to this checkout; never to the machine. A machine-wide lock made four
//! verify runs of four checkouts (the daemon's checks of three ships and a gate) wait on each
//! other's pty tests until the ten-minute deadline failed them (TOKNILA-128's gate, 2026-11-15).

use std::collections::hash_map::DefaultHasher;
use std::fs::OpenOptions;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

/// How long a test waits for its turn before it fails.
const TURN_DEADLINE: Duration = Duration::from_secs(600);
/// A lock file older than this belongs to a test that was killed; it is taken over.
const STALE_AFTER: Duration = Duration::from_secs(300);

fn lock_path() -> PathBuf {
    // The run's root when verify set one, else this checkout: two checkouts never share a lock.
    let scope = std::env::var_os("TOKNILA_TEST_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            // The workspace root, the same for the daemon's and the cli's test binaries.
            let crates = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("..");
            crates.canonicalize().unwrap_or(crates)
        });
    let mut hasher = DefaultHasher::new();
    scope.hash(&mut hasher);
    std::env::temp_dir().join(format!("toknila-pty-tests-{:016x}.lock", hasher.finish()))
}

/// Holds the pty lock until dropped.
#[derive(Debug)]
pub struct PtyTurn {
    path: PathBuf,
}

thread_local! {
    /// Set while this thread holds a turn taken by [`hold`].
    static HOLDING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

impl Drop for PtyTurn {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
        HOLDING.with(|h| h.set(false));
    }
}

/// Waits for the pty lock (a file in the temporary folder, shared by every checkout on this
/// computer) and holds it; fails the test after ten minutes. A thread that already holds a turn
/// and asks again would wait for itself until the lock counts as stale (five minutes, on every
/// system, TOKNILA-165), so it panics at once instead.
pub fn hold() -> PtyTurn {
    assert!(
        !HOLDING.with(|h| h.get()),
        "this thread already holds the pty turn: drop the first rig before taking a second"
    );
    let turn = hold_for(TURN_DEADLINE).expect("no turn for the pty tests within the deadline");
    HOLDING.with(|h| h.set(true));
    turn
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
