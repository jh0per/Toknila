//! Where Toknila keeps its files.
//!
//! `TOKNILA_HOME` wins (tests, several instances side by side). Otherwise `%APPDATA%\toknila` on
//! Windows and `~/.toknila` elsewhere.

use std::path::{Path, PathBuf};

/// Environment variable that overrides the home directory.
pub const HOME_ENV: &str = "TOKNILA_HOME";

/// The files under one Toknila home.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    pub home: PathBuf,
}

impl Paths {
    /// Resolves the home directory from the environment and the platform.
    pub fn resolve() -> Result<Paths, String> {
        if let Some(home) = std::env::var_os(HOME_ENV).filter(|v| !v.is_empty()) {
            return Ok(Paths {
                home: PathBuf::from(home),
            });
        }
        let home = if cfg!(windows) {
            dirs::data_dir().map(|d| d.join("toknila"))
        } else {
            dirs::home_dir().map(|d| d.join(".toknila"))
        };
        home.map(|home| Paths { home })
            .ok_or_else(|| "cannot find the home directory".to_owned())
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

    /// Rotated daemon logs.
    pub fn logs(&self) -> PathBuf {
        self.home.join("logs")
    }
}
