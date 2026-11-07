//! Where Toknila keeps its files.
//!
//! `TOKNILA_HOME` wins (tests, several instances side by side). Otherwise the platform's data
//! directory: `~/Library/Application Support/toknila` on macOS, `%APPDATA%\toknila` on Windows,
//! and `~/.toknila` on Linux. A macOS home from before 0.2 (`~/.toknila`) is moved to the new
//! place on first use, unless a daemon may still be using it.
//!
//! Ships (worktrees) do not live in the data home: its path has a space on macOS, which makes
//! agents backslash-escape paths. They go to `~/.toknila-ships` (never `~/.toknila`, which is the
//! legacy data home), or to `TOKNILA_SHIPS_DIR`. With an explicit `TOKNILA_HOME` they stay under
//! that home, so a test or a second instance never writes into the person's real ships.

use std::path::{Path, PathBuf};

/// Environment variable that overrides the home directory.
pub const HOME_ENV: &str = "TOKNILA_HOME";

/// Environment variable that overrides the folder of the ships.
pub const SHIPS_ENV: &str = "TOKNILA_SHIPS_DIR";

/// Folder name of the default ships root, next to the person's home (no space in it).
const SHIPS_DIR_NAME: &str = ".toknila-ships";

/// The files under one Toknila home.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    pub home: PathBuf,
    /// Where new ships are made; recorded ships keep the path they were made with.
    pub ships: PathBuf,
}

/// The ships root: the override, else `~/.toknila-ships`, else (no home folder known) under
/// `home`.
pub(crate) fn ships_root(over: Option<PathBuf>, user_home: Option<&Path>, home: &Path) -> PathBuf {
    over.or_else(|| user_home.map(|h| h.join(SHIPS_DIR_NAME)))
        .unwrap_or_else(|| home.join("ships"))
}

fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// Where the home of this platform is, and the older place it may still be in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Layout {
    pub home: PathBuf,
    pub legacy: Option<PathBuf>,
}

/// The platform layout from the user's home and data directories.
pub(crate) fn layout(
    os: &str,
    home_dir: Option<PathBuf>,
    data_dir: Option<PathBuf>,
) -> Option<Layout> {
    let dot = home_dir.map(|h| h.join(".toknila"));
    match os {
        "macos" => Some(Layout {
            home: data_dir?.join("toknila"),
            legacy: dot,
        }),
        "windows" => Some(Layout {
            home: data_dir?.join("toknila"),
            legacy: None,
        }),
        _ => Some(Layout {
            home: dot?,
            legacy: None,
        }),
    }
}

/// The home to use: `layout.home`, after moving a legacy home there when it is safe. A legacy
/// home with a `daemon.json` may belong to a running daemon; it is used where it is and moved
/// on a later start, after that daemon stopped.
pub(crate) fn settle_home(layout: &Layout) -> std::io::Result<PathBuf> {
    let Some(legacy) = layout.legacy.as_deref().filter(|l| l.is_dir()) else {
        return Ok(layout.home.clone());
    };
    if layout.home.exists() {
        return Ok(layout.home.clone());
    }
    if Paths::at(legacy).daemon_json().exists() {
        return Ok(legacy.to_owned());
    }
    if let Some(parent) = layout.home.parent() {
        std::fs::create_dir_all(parent)?;
    }
    match std::fs::rename(legacy, &layout.home) {
        Ok(()) => Ok(layout.home.clone()),
        // Another volume (rename cannot cross it): keep the old place rather than copy a live db.
        Err(_) if legacy.is_dir() && !layout.home.exists() => Ok(legacy.to_owned()),
        Err(e) => Err(e),
    }
}

impl Paths {
    /// Resolves the home directory from the environment and the platform.
    pub fn resolve() -> Result<Paths, String> {
        if let Some(home) = env_path(HOME_ENV) {
            let ships = env_path(SHIPS_ENV).unwrap_or_else(|| home.join("ships"));
            return Ok(Paths { home, ships });
        }
        let layout = layout(std::env::consts::OS, dirs::home_dir(), dirs::data_dir())
            .ok_or_else(|| "cannot find the home directory".to_owned())?;
        let home = settle_home(&layout).map_err(|e| {
            format!(
                "cannot move {} to {}: {e}",
                layout.legacy.unwrap_or_default().display(),
                layout.home.display()
            )
        })?;
        let ships = ships_root(env_path(SHIPS_ENV), dirs::home_dir().as_deref(), &home);
        Ok(Paths { home, ships })
    }

    /// Uses an explicit directory.
    pub fn at(home: impl AsRef<Path>) -> Paths {
        let home = home.as_ref().to_owned();
        Paths {
            ships: home.join("ships"),
            home,
        }
    }

    /// The SQLite database.
    pub fn db(&self) -> PathBuf {
        self.home.join("toknila.db")
    }

    /// Port, token and pid of the running daemon.
    pub fn daemon_json(&self) -> PathBuf {
        self.home.join("daemon.json")
    }

    /// Address and token of the last cleanly stopped daemon, reused by the next start.
    pub fn daemon_last_json(&self) -> PathBuf {
        self.home.join("daemon.last.json")
    }

    /// Agent profiles: one `<name>.toml` per agent, next to the built-in `sim` and `claude-code`.
    pub fn profiles(&self) -> PathBuf {
        self.home.join("profiles")
    }

    /// Worktrees of ships: `<ships root>/<project>/<chain>/<slug>`.
    pub fn ships(&self) -> PathBuf {
        self.ships.clone()
    }

    /// Output of agent runs: `runs/<ticket id>/<run id>.log`.
    pub fn runs(&self) -> PathBuf {
        self.home.join("runs")
    }

    /// Rotated daemon logs.
    pub fn logs(&self) -> PathBuf {
        self.home.join("logs")
    }
}

#[cfg(test)]
mod tests;
