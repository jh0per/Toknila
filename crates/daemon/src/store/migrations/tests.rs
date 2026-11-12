use super::*;

const A: (&str, &str) = (
    "a_first.sql",
    "-- name: card-a\nCREATE TABLE a (x INTEGER);\n",
);
const B: (&str, &str) = (
    "b_second.sql",
    "-- name: card-b\nCREATE TABLE b (x INTEGER);\n",
);

fn run(conn: &mut Connection, files: &[(&str, &str)]) -> Result<(), StoreError> {
    let dir = tempfile::tempdir().expect("dir");
    migrate(conn, dir.path(), &parse(files)?)
}

fn recorded(conn: &Connection) -> Vec<String> {
    let mut s = conn
        .prepare("SELECT name FROM schema_migrations ORDER BY applied_at, rowid")
        .expect("prepare");
    s.query_map([], |r| r.get(0))
        .expect("query")
        .collect::<Result<_, _>>()
        .expect("rows")
}

fn has_table(conn: &Connection, name: &str) -> bool {
    conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM sqlite_master WHERE name = ?1)",
        [name],
        |r| r.get(0),
    )
    .expect("exists")
}

#[test]
fn a_fresh_store_runs_every_migration_and_records_their_names() {
    let mut conn = Connection::open_in_memory().expect("open");
    let list = all().expect("list");
    let dir = tempfile::tempdir().expect("dir");
    migrate(&mut conn, dir.path(), &list).expect("migrate");
    let names: Vec<&str> = list.iter().map(|m| m.name.as_str()).collect();
    assert_eq!(recorded(&conn), names);
    assert!(has_table(&conn, "design_pins"));
    assert!(
        !dir.path().join("x").exists() && std::fs::read_dir(dir.path()).expect("ls").count() == 0
    );
}

#[test]
fn an_old_store_is_seeded_by_name_and_only_the_new_migrations_run() {
    let mut conn = Connection::open_in_memory().expect("open");
    old_store(&conn, 24);
    conn.execute(
        "INSERT INTO projects (id, key, name, work_branch, release_branch, default_profile, default_review_mode)
         VALUES ('p', 'DEMO', 'Demo', 'd', 'm', 'sim', 'human')",
        [],
    )
    .expect("project");
    // The 24 legacy files, then one new one: the files of later cards are not part of the story.
    let mut files: Vec<(&str, &str)> = FILES
        .iter()
        .copied()
        .filter(|(file, _)| file.starts_with("00"))
        .collect();
    assert_eq!(files.len(), 24);
    files.push(A);
    let list = parse(&files).expect("list");
    let dir = tempfile::tempdir().expect("dir");
    migrate(&mut conn, dir.path(), &list).expect("migrate");
    let names = recorded(&conn);
    assert_eq!(names.len(), 25);
    assert_eq!(names[0], "legacy-01-phase0-schema");
    assert_eq!(names[23], "legacy-24-design-pins");
    assert_eq!(names[24], "card-a");
    assert!(has_table(&conn, "a"));
    // The project survived, and the backup is named after the 24 that had run.
    let kept: Vec<String> = std::fs::read_dir(dir.path())
        .expect("ls")
        .map(|e| e.expect("e").file_name().to_string_lossy().into_owned())
        .collect();
    assert!(
        kept.iter().all(|n| n.starts_with("toknila-v24-")),
        "{kept:?}"
    );
    assert_eq!(kept.len(), 1);
}

#[test]
fn a_half_old_store_runs_the_rest_once() {
    let mut conn = Connection::open_in_memory().expect("open");
    old_store(&conn, 3);
    let list = all().expect("list");
    let dir = tempfile::tempdir().expect("dir");
    migrate(&mut conn, dir.path(), &list).expect("first");
    migrate(&mut conn, dir.path(), &list).expect("second");
    assert_eq!(recorded(&conn).len(), list.len());
}

#[test]
fn a_recorded_name_never_runs_again() {
    let mut conn = Connection::open_in_memory().expect("open");
    run(&mut conn, &[A]).expect("first");
    conn.execute_batch("DROP TABLE a").expect("drop");
    run(&mut conn, &[A, B]).expect("second");
    assert!(!has_table(&conn, "a"), "card-a ran twice");
    assert!(has_table(&conn, "b"));
    assert_eq!(recorded(&conn), ["card-a", "card-b"]);
}

#[test]
fn a_duplicate_name_is_refused_naming_both_files() {
    let twin = (
        "c_twin.sql",
        "-- name: card-a\nCREATE TABLE c (x INTEGER);\n",
    );
    let err = parse(&[A, B, twin]).expect_err("refused");
    let text = err.to_string();
    assert!(
        text.contains("card-a") && text.contains("a_first.sql") && text.contains("c_twin.sql"),
        "{text}"
    );
    let mut conn = Connection::open_in_memory().expect("open");
    assert!(run(&mut conn, &[A, B, twin]).is_err());
    assert!(!has_table(&conn, "schema_migrations"), "nothing ran");
}

#[test]
fn migrations_run_in_list_order() {
    let mut conn = Connection::open_in_memory().expect("open");
    let later = ("z.sql", "-- name: card-z\nINSERT INTO log VALUES ('z');\n");
    let first = (
        "m.sql",
        "-- name: card-m\nCREATE TABLE log (v TEXT);\nINSERT INTO log VALUES ('m');\n",
    );
    run(&mut conn, &[first, later]).expect("migrate");
    let log: Vec<String> = conn
        .prepare("SELECT v FROM log ORDER BY rowid")
        .expect("prepare")
        .query_map([], |r| r.get(0))
        .expect("query")
        .collect::<Result<_, _>>()
        .expect("rows");
    assert_eq!(log, ["m", "z"]);
    assert_eq!(recorded(&conn), ["card-m", "card-z"]);
}

#[test]
fn a_failing_migration_rolls_back_alone_and_stops_the_rest() {
    let mut conn = Connection::open_in_memory().expect("open");
    let bad = (
        "b.sql",
        "-- name: card-bad\nCREATE TABLE half (x INTEGER);\nNOT SQL;\n",
    );
    assert!(run(&mut conn, &[A, bad, B]).is_err());
    assert_eq!(recorded(&conn), ["card-a"]);
    assert!(!has_table(&conn, "half"));
    assert!(!has_table(&conn, "b"));
}

#[test]
fn a_file_without_a_name_line_is_refused() {
    let err = parse(&[("x.sql", "CREATE TABLE x (y INTEGER);")]).expect_err("refused");
    assert!(err.to_string().contains("x.sql"), "{err}");
}

#[test]
fn the_shipped_names_are_unique_and_the_legacy_ones_come_first() {
    let list = all().expect("unique");
    assert!(list.len() >= 24);
    for (i, m) in list.iter().take(24).enumerate() {
        assert!(
            m.name.starts_with(&format!("legacy-{:02}-", i + 1)),
            "{}",
            m.name
        );
    }
    for m in list.iter().skip(24) {
        assert!(
            !m.file.starts_with(|c: char| c.is_ascii_digit()),
            "{}",
            m.file
        );
    }
}

/// The legacy names, in the order the old positional list ran them.
const LEGACY: [&str; 24] = [
    "legacy-01-phase0-schema",
    "legacy-02-wildcard-cards",
    "legacy-03-threads",
    "legacy-04-done-out-of-order",
    "legacy-05-parallel-agents",
    "legacy-06-review",
    "legacy-07-pull-request-mode",
    "legacy-08-token-usage",
    "legacy-09-check-command",
    "legacy-10-archived-projects",
    "legacy-11-chain-source",
    "legacy-12-project-models",
    "legacy-13-command-policy",
    "legacy-14-canvas-position",
    "legacy-15-design-first",
    "legacy-16-reviewer-web-domains",
    "legacy-17-max-review-rounds",
    "legacy-18-no-checks-timeout",
    "legacy-19-canvas-size",
    "legacy-20-bump-command",
    "legacy-21-ticket-designs",
    "legacy-22-suggestions",
    "legacy-23-chain-version",
    "legacy-24-design-pins",
];

/// Files that sort after the legacy ones; `0024_` is the last legacy prefix.
fn sorts_after_legacy(file: &str) -> bool {
    file > "0024_~"
}

#[test]
fn the_first_24_migrations_are_the_legacy_ones_in_order() {
    let list = all().expect("list");
    let names: Vec<&str> = list.iter().take(24).map(|m| m.name.as_str()).collect();
    assert_eq!(names, LEGACY, "seeding an old store takes the first N");
    for m in &list[24..] {
        assert!(
            sorts_after_legacy(&m.file),
            "{} sorts among the legacy migrations; start its file name with a letter",
            m.file
        );
        assert!(!m.name.starts_with("legacy-"), "{}: legacy name", m.file);
    }
}

#[test]
fn a_file_that_does_not_sort_after_the_legacy_ones_is_caught() {
    for bad in ["0010_x.sql", "0024_x.sql", "0001_a.sql"] {
        assert!(!sorts_after_legacy(bad), "{bad}");
    }
    for ok in ["a_card.sql", "toknila-50.sql", "0025_x.sql"] {
        assert!(sorts_after_legacy(ok), "{ok}");
    }
}
