//! What other runs left: daemons and fake agents with a temporary home, alive for hours. Said as
//! a warning by `verify` and `e2e-web`, ended by `cargo xtask leaks --kill-old` on request.

use super::{Agent, Daemon, KILL_OLD_AFTER, kill_tree, listing, scopes, under};
use std::path::{Path, PathBuf};

/// A folder a test makes its home in: under one of `temps` (the OS temp dir), or inside a
/// `.toknila-ships` worktree (a ship's own tests). Whether it still exists does not matter.
fn temporary(path: &Path, temps: &[PathBuf]) -> bool {
    under(path, temps) || path.components().any(|c| c.as_os_str() == SHIPS_FOLDER)
}

/// The folder the ships' worktrees live in (under the user's home).
const SHIPS_FOLDER: &str = ".toknila-ships";

/// One daemon or fake agent another run left: pid, age in seconds and the words for it
/// ("a daemon from <home> (home gone)").
#[derive(Debug, PartialEq)]
pub struct Old {
    pub pid: u32,
    pub age: u64,
    pub what: String,
}

/// Whether the folder is there.
pub fn exists(path: &Path) -> bool {
    path.exists()
}

/// The home as said in words: its path, and "(home gone)" when `exists` says the folder is not there.
fn from_home(home: &Path, exists: &dyn Fn(&Path) -> bool) -> String {
    if exists(home) {
        home.display().to_string()
    } else {
        format!("{} (home gone)", home.display())
    }
}

/// Daemons with a temporary home (see [`temporary`], the folder may be gone) and fake agents that
/// are at least `min_age` seconds old and not under `mine` (this run's root): what another run
/// left. An agent whose home is unknown is named by its command line.
pub fn old_ones(
    daemons: &[Daemon],
    agents: &[Agent],
    mine: &[PathBuf],
    temps: &[PathBuf],
    min_age: u64,
    exists: &dyn Fn(&Path) -> bool,
) -> Vec<Old> {
    let mut out = Vec::new();
    for d in daemons {
        if d.age >= min_age && temporary(&d.home, temps) && !under(&d.home, mine) {
            out.push(Old {
                pid: d.pid,
                age: d.age,
                what: format!("a daemon from {}", from_home(&d.home, exists)),
            });
        }
    }
    for a in agents {
        let theirs = a
            .home
            .as_ref()
            .is_none_or(|h| temporary(h, temps) && !under(h, mine));
        if a.age >= min_age && theirs {
            let from = a
                .home
                .as_ref()
                .map_or_else(|| format!("`{}`", a.command), |h| from_home(h, exists));
            out.push(Old {
                pid: a.pid,
                age: a.age,
                what: format!("a fake agent from {from}"),
            });
        }
    }
    out
}

/// What [`old_ones`] finds, said as a warning: another run left it, and it is not killed.
pub fn old_leftovers(
    daemons: &[Daemon],
    agents: &[Agent],
    mine: &[PathBuf],
    temps: &[PathBuf],
    min_age: u64,
    exists: &dyn Fn(&Path) -> bool,
) -> Vec<String> {
    old_ones(daemons, agents, mine, temps, min_age, exists)
        .into_iter()
        .map(|o| {
            format!(
                "{}, alive for {} h; another run left it (pid {}, not killed)",
                o.what,
                o.age / 3600,
                o.pid
            )
        })
        .collect()
}

/// `cargo xtask leaks --kill-old`: ends every leftover [`old_ones`] finds that is older than
/// [`KILL_OLD_AFTER`], printing each. Only on request; verify and e2e-web never call it.
pub fn kill_old() -> Result<(), String> {
    let Some((all, agents)) = listing() else {
        return Err("leaks --kill-old: the process list cannot be read".to_owned());
    };
    let old = old_ones(&all, &agents, &[], &scopes(None), KILL_OLD_AFTER, &exists);
    if old.is_empty() {
        eprintln!("xtask: no test daemon or fake agent older than a day to end");
        return Ok(());
    }
    for o in old {
        eprintln!(
            "xtask: ending {}, alive for {} h (pid {})",
            o.what,
            o.age / 3600,
            o.pid
        );
        kill_tree(o.pid);
    }
    Ok(())
}
