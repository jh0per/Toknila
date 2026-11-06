//! Where Toknila keeps its files.
//!
//! `TOKNILA_HOME` wins (tests, several instances side by side). Otherwise the platform's data
//! directory: `~/Library/Application Support/toknila` on macOS, `%APPDATA%\toknila` on Windows,
//! and `~/.toknila` on Linux. A macOS home from before 0.2 (`~/.toknila`) is moved to the new
//! place on first use, unless a daemon may still be using it.

use std::path::{Path, PathBuf};

/// Environment variable that overrides the home directory.
pub const HOME_ENV: &str = "TOKNILA_HOME";

/// The files under one Toknila home.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    pub home: PathBuf,
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
        if let Some(home) = std::env::var_os(HOME_ENV).filter(|v| !v.is_empty()) {
            return Ok(Paths {
                home: PathBuf::from(home),
            });
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
        Ok(Paths { home })
    }

    /// Uses an explicit directory.
    pub fn at(home: impl AsRef<Path>) -> Paths {
        Paths {
            home: home.as_ref().to_owned(),
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

    /// Worktrees of ships: `ships/<project>/<chain>/<slug>`.
    pub fn ships(&self) -> PathBuf {
        self.home.join("ships")
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
