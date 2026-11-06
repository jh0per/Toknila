//! Store failures.

use std::path::{Path, PathBuf};

/// Store failures.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("database is newer than this build (schema {found}, supported {supported})")]
    TooNew { found: i64, supported: i64 },
    #[error("cannot back up the database to {path} before migrating it: {reason}; the migration did not run", path = path.display())]
    BackupFailed { path: PathBuf, reason: String },
    #[error(
        "the database {db} failed its integrity check ({detail}). {restore}",
        db = db.display(),
        restore = restore_hint(backup.as_deref(), db)
    )]
    IntegrityCheck {
        db: PathBuf,
        detail: String,
        backup: Option<PathBuf>,
    },
    #[error("command panicked: {0}")]
    Panicked(String),
    #[error("store task stopped")]
    Closed,
    #[error("{0}")]
    Corrupt(String),
}

/// What to do about a damaged database: restore the newest backup over it.
fn restore_hint(backup: Option<&Path>, db: &Path) -> String {
    match backup {
        Some(b) => format!(
            "Stop the daemon, move {db} (and its -wal and -shm files) away and copy the newest backup {b} to {db}, then start again.",
            db = db.display(),
            b = b.display()
        ),
        None => {
            "There is no backup yet; move the file away to start with an empty database.".to_owned()
        }
    }
}
