//! Safety around the database file: a consistent copy before a schema migration, the newest
//! backups kept, and the integrity check at start.

use std::path::{Path, PathBuf};

use rusqlite::Connection;

use super::StoreError;

/// How many backups stay in the backups folder.
pub const KEEP: usize = 5;

const PREFIX: &str = "toknila-v";
const SUFFIX: &str = ".sqlite";

/// Copies the database (a SQLite `VACUUM INTO`, consistent even in WAL mode) to
/// `<dir>/toknila-v<from>-<UTC timestamp>.sqlite`, then drops the oldest beyond [`KEEP`]. The
/// copy is written under a temporary name and renamed, so a half-written file is never taken
/// for a backup.
pub(super) fn before_migration(
    conn: &Connection,
    dir: &Path,
    from: i64,
) -> Result<PathBuf, StoreError> {
    let stamp: String = conn
        .query_row("SELECT strftime('%Y%m%dT%H%M%SZ', 'now')", [], |r| r.get(0))
        .map_err(|e| failed(dir, &e))?;
    let target = dir.join(format!("{PREFIX}{from}-{stamp}{SUFFIX}"));
    std::fs::create_dir_all(dir).map_err(|e| failed(&target, &e))?;
    if target.exists() {
        return Err(failed(&target, &"the file exists already"));
    }
    let partial = target.with_extension("sqlite.part");
    let _ = std::fs::remove_file(&partial);
    let copied = partial
        .to_str()
        .ok_or_else(|| failed(&target, &"the path is not valid UTF-8"))
        .and_then(|p| {
            conn.execute("VACUUM INTO ?1", [p])
                .map_err(|e| failed(&target, &e))
        })
        .and_then(|_| std::fs::rename(&partial, &target).map_err(|e| failed(&target, &e)));
    if let Err(e) = copied {
        let _ = std::fs::remove_file(&partial);
        return Err(e);
    }
    prune(dir);
    Ok(target)
}

fn failed(path: &Path, reason: &dyn std::fmt::Display) -> StoreError {
    StoreError::BackupFailed {
        path: path.to_owned(),
        reason: reason.to_string(),
    }
}

/// The backups in `dir`, oldest first (by the timestamp in their names).
fn list(dir: &Path) -> Vec<PathBuf> {
    let mut found: Vec<(String, PathBuf)> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            let stem = name.strip_prefix(PREFIX)?.strip_suffix(SUFFIX)?;
            let (_, stamp) = stem.split_once('-')?;
            Some((stamp.to_owned(), e.path()))
        })
        .collect();
    found.sort();
    found.into_iter().map(|(_, p)| p).collect()
}

fn prune(dir: &Path) {
    let all = list(dir);
    for old in all.iter().take(all.len().saturating_sub(KEEP)) {
        if let Err(e) = std::fs::remove_file(old) {
            tracing::warn!(path = %old.display(), error = %e, "cannot remove an old backup");
        }
    }
}

/// Runs `PRAGMA quick_check`; a database that is not sound is refused, naming the newest backup.
pub(super) fn check(conn: &Connection, db: &Path, dir: &Path) -> Result<(), StoreError> {
    let verdict = conn
        .prepare("PRAGMA quick_check")
        .and_then(|mut s| s.query_map([], |r| r.get::<_, String>(0))?.collect())
        .unwrap_or_else(|e| vec![e.to_string()]);
    if verdict == ["ok"] {
        return Ok(());
    }
    Err(StoreError::IntegrityCheck {
        db: db.to_owned(),
        detail: verdict.into_iter().take(5).collect::<Vec<_>>().join("; "),
        backup: list(dir).pop(),
    })
}

#[cfg(test)]
mod tests;
