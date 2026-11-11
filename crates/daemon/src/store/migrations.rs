//! Schema migrations, by name. Each is a file in `migrations/` whose first line is
//! `-- name: <slug>`; a build step collects the files in file-name order (the first 24 are the
//! legacy ones, `0001_` to `0024_`; a new file starts with a letter, so it sorts after them).
//! The `schema_migrations` table records the names that ran; on start the ones not recorded run,
//! each in its own transaction. Never edit a released migration; add a file.

use std::collections::HashSet;
use std::path::Path;

use rusqlite::{Connection, params};

use super::{StoreError, backup};

include!(concat!(env!("OUT_DIR"), "/migration_files.rs"));

/// One migration: its unique name, the file it came from and its SQL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Migration {
    pub name: String,
    pub file: String,
    pub sql: String,
}

/// The migrations of this build, in the order they run.
pub(super) fn all() -> Result<Vec<Migration>, StoreError> {
    parse(FILES)
}

/// Reads the name of each `(file, sql)` and refuses a name used twice, naming both files.
pub(super) fn parse(files: &[(&str, &str)]) -> Result<Vec<Migration>, StoreError> {
    let mut out: Vec<Migration> = Vec::with_capacity(files.len());
    for (file, sql) in files {
        let bad = |reason: &str| StoreError::BadMigration {
            file: (*file).to_owned(),
            reason: reason.to_owned(),
        };
        let name = sql
            .lines()
            .next()
            .and_then(|l| l.strip_prefix("-- name:"))
            .map(str::trim)
            .ok_or_else(|| bad("the first line must be `-- name: <slug>`"))?;
        let slug = !name.is_empty()
            && name.starts_with(|c: char| c.is_ascii_alphanumeric())
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
        if !slug {
            return Err(bad(
                "the name must be a slug of letters, digits, `-`, `_`, `.`",
            ));
        }
        if let Some(first) = out.iter().find(|m| m.name == name) {
            return Err(StoreError::DuplicateMigration {
                name: name.to_owned(),
                first: first.file.clone(),
                second: (*file).to_owned(),
            });
        }
        out.push(Migration {
            name: name.to_owned(),
            file: (*file).to_owned(),
            sql: (*sql).to_owned(),
        });
    }
    Ok(out)
}

/// Runs the migrations whose names are not recorded, in list order. A store from before names
/// (`user_version` N, no `schema_migrations` table) is seeded with the first N names instead of
/// running them again.
pub(super) fn migrate(
    conn: &mut Connection,
    backups: &Path,
    list: &[Migration],
) -> Result<(), StoreError> {
    let has_table: bool = conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'schema_migrations')",
        [],
        |r| r.get(0),
    )?;
    let mut done: HashSet<String> = HashSet::new();
    let mut seed: &[Migration] = &[];
    if has_table {
        let mut s = conn.prepare("SELECT name FROM schema_migrations")?;
        done = s.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?;
        let known: HashSet<&str> = list.iter().map(|m| m.name.as_str()).collect();
        let mut unknown: Vec<&String> = done
            .iter()
            .filter(|n| !known.contains(n.as_str()))
            .collect();
        unknown.sort();
        if !unknown.is_empty() {
            let names: Vec<&str> = unknown.iter().map(|n| n.as_str()).collect();
            return Err(StoreError::TooNew {
                unknown: names.join(", "),
            });
        }
    } else {
        let old: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        let old = usize::try_from(old).unwrap_or(0);
        if old > list.len() {
            return Err(StoreError::TooNew {
                unknown: format!("schema version {old}, this build knows {}", list.len()),
            });
        }
        seed = &list[..old];
    }
    let existing = done.len() + seed.len();
    let pending = list
        .iter()
        .any(|m| !done.contains(&m.name) && !seed.iter().any(|s| s.name == m.name));
    // A fresh database has nothing to lose; an existing one is copied first.
    if existing > 0 && pending {
        backup::before_migration(conn, backups, i64::try_from(existing).unwrap_or(i64::MAX))?;
    }
    if !has_table {
        let tx = conn.transaction()?;
        tx.execute_batch(
            "CREATE TABLE schema_migrations (name TEXT PRIMARY KEY, applied_at INTEGER NOT NULL)",
        )?;
        for m in seed {
            record(&tx, &m.name)?;
            done.insert(m.name.clone());
        }
        tx.commit()?;
    }
    for m in list.iter().filter(|m| !done.contains(&m.name)) {
        let tx = conn.transaction()?;
        tx.execute_batch(&m.sql)?;
        record(&tx, &m.name)?;
        tx.commit()?;
    }
    Ok(())
}

/// Test fixture: a store as a build before names left it, the first `n` migrations applied and
/// `user_version` set to `n`.
#[cfg(test)]
pub(super) fn old_store(conn: &Connection, n: usize) {
    for m in &all().expect("list")[..n] {
        conn.execute_batch(&m.sql).expect("legacy migration");
    }
    conn.pragma_update(None, "user_version", i64::try_from(n).expect("small"))
        .expect("version");
}

#[cfg(test)]
mod tests;

/// Records that `name` ran, now (milliseconds since the epoch, like every timestamp).
fn record(tx: &rusqlite::Transaction<'_>, name: &str) -> Result<(), StoreError> {
    tx.execute(
        "INSERT INTO schema_migrations (name, applied_at) VALUES (?1, CAST(strftime('%s', 'now') AS INTEGER) * 1000)",
        params![name],
    )?;
    Ok(())
}
