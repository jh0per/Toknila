use rusqlite::Connection;

use super::super::schema::MIGRATIONS;
use super::super::{SCHEMA_VERSION, Store, StoreError};
use super::*;

/// A database as an older build left it: the first `version` migrations applied, plus a project.
fn old_db(path: &Path, version: usize) {
    let conn = Connection::open(path).expect("open");
    conn.pragma_update(None, "journal_mode", "WAL")
        .expect("wal");
    for sql in &MIGRATIONS[..version] {
        conn.execute_batch(sql).expect("migration");
    }
    conn.pragma_update(None, "user_version", version as i64)
        .expect("version");
    conn.execute(
        "INSERT INTO projects (id, key, name, work_branch, release_branch, default_profile, default_review_mode)
         VALUES ('p1', 'DEMO', 'Demo', 'develop', 'main', 'sim', 'human')",
        [],
    )
    .expect("project");
}

fn names(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .map(|d| {
            d.filter_map(Result::ok)
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

fn version_of(path: &Path) -> i64 {
    Connection::open(path)
        .expect("open")
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .expect("version")
}

fn projects(path: &Path) -> Vec<String> {
    let conn = Connection::open(path).expect("open");
    let mut s = conn.prepare("SELECT key FROM projects").expect("prepare");
    s.query_map([], |r| r.get(0))
        .expect("query")
        .collect::<Result<_, _>>()
        .expect("rows")
}

#[test]
fn migrating_an_older_database_backs_it_up_and_keeps_the_data() {
    let dir = tempfile::tempdir().expect("dir");
    let db = dir.path().join("toknila.db");
    old_db(&db, 1);
    let store = Store::open(&db).expect("open");
    drop(store);
    let kept = names(&dir.path().join("backups"));
    assert_eq!(kept.len(), 1, "{kept:?}");
    assert!(
        kept[0].starts_with("toknila-v1-") && kept[0].ends_with("Z.sqlite"),
        "{kept:?}"
    );
    let backup = dir.path().join("backups").join(&kept[0]);
    assert_eq!(version_of(&backup), 1);
    assert_eq!(projects(&backup), ["DEMO"]);
    assert_eq!(version_of(&db), SCHEMA_VERSION);
    assert_eq!(projects(&db), ["DEMO"]);
}

#[test]
fn a_fresh_database_needs_no_backup() {
    let dir = tempfile::tempdir().expect("dir");
    Store::open(&dir.path().join("toknila.db")).expect("open");
    assert!(names(&dir.path().join("backups")).is_empty());
}

#[test]
fn a_database_that_is_current_needs_no_backup() {
    let dir = tempfile::tempdir().expect("dir");
    old_db(&dir.path().join("toknila.db"), MIGRATIONS.len());
    Store::open(&dir.path().join("toknila.db")).expect("open");
    assert!(names(&dir.path().join("backups")).is_empty());
}

#[test]
fn a_failing_backup_stops_the_migration_and_leaves_the_database_alone() {
    let dir = tempfile::tempdir().expect("dir");
    let db = dir.path().join("toknila.db");
    old_db(&db, 1);
    // A file where the backups folder should be.
    std::fs::write(dir.path().join("backups"), "in the way").expect("block");
    let err = Store::open(&db).expect_err("refused");
    let StoreError::BackupFailed { path, .. } = &err else {
        panic!("wrong error: {err}");
    };
    assert!(path.starts_with(dir.path().join("backups")), "{path:?}");
    assert!(err.to_string().contains("backups"), "{err}");
    assert_eq!(version_of(&db), 1);
    let conn = Connection::open(&db).expect("open");
    let columns: i64 = conn
        .query_row(
            "SELECT count(*) FROM pragma_table_info('tickets') WHERE name = 'wildcard_json'",
            [],
            |r| r.get(0),
        )
        .expect("columns");
    assert_eq!(columns, 0);
    assert_eq!(projects(&db), ["DEMO"]);
}

#[test]
fn only_the_newest_five_backups_stay() {
    let dir = tempfile::tempdir().expect("dir");
    let backups = dir.path().join("backups");
    std::fs::create_dir_all(&backups).expect("dir");
    for day in 1..=7 {
        std::fs::write(
            backups.join(format!("toknila-v1-2025010{day}T000000Z.sqlite")),
            "x",
        )
        .expect("old backup");
    }
    std::fs::write(backups.join("notes.txt"), "mine").expect("foreign file");
    old_db(&dir.path().join("toknila.db"), 1);
    Store::open(&dir.path().join("toknila.db")).expect("open");
    let kept = names(&backups);
    assert_eq!(kept.len(), 6, "five backups and the foreign file: {kept:?}");
    assert!(kept.contains(&"notes.txt".to_owned()));
    for gone in ["20250101", "20250102", "20250103"] {
        assert!(!kept.iter().any(|n| n.contains(gone)), "{kept:?}");
    }
    assert!(kept.iter().any(|n| n.contains("20250107")));
    assert!(
        kept.iter()
            .any(|n| n.starts_with("toknila-v1-202") && !n.contains("2025010")),
        "{kept:?}"
    );
}

#[test]
fn a_damaged_database_is_refused_naming_the_newest_backup() {
    let dir = tempfile::tempdir().expect("dir");
    let db = dir.path().join("toknila.db");
    old_db(&db, MIGRATIONS.len());
    {
        let conn = Connection::open(&db).expect("open");
        conn.execute_batch("CREATE INDEX events_at ON events(at)")
            .expect("index");
        for i in 0..3000 {
            conn.execute(
                "INSERT INTO events (at, actor, entity, entity_id, kind, payload_json) VALUES (?1, 'human', 'ticket', 'x', 'k', '{}')",
                [i],
            )
            .expect("event");
        }
        conn.pragma_update(None, "journal_mode", "DELETE")
            .expect("checkpoint");
    }
    let backups = dir.path().join("backups");
    std::fs::create_dir_all(&backups).expect("dir");
    std::fs::write(backups.join("toknila-v3-20250101T000000Z.sqlite"), "x").expect("old");
    std::fs::write(backups.join("toknila-v3-20250202T000000Z.sqlite"), "x").expect("new");
    let mut bytes = std::fs::read(&db).expect("read");
    let len = bytes.len();
    for b in &mut bytes[len * 3 / 4..len * 3 / 4 + 4096] {
        *b = 0xFF;
    }
    std::fs::write(&db, bytes).expect("damage");
    let err = Store::open(&db).expect_err("refused");
    let StoreError::IntegrityCheck { backup, .. } = &err else {
        panic!("wrong error: {err}");
    };
    assert_eq!(
        backup.as_deref(),
        Some(backups.join("toknila-v3-20250202T000000Z.sqlite").as_path())
    );
    let text = err.to_string();
    assert!(
        text.contains("20250202T000000Z") && text.contains("copy the newest backup"),
        "{text}"
    );
}
