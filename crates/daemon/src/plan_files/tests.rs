//! Reading paths for an import and scanning a repository, on temporary folders.

use std::path::{Path, PathBuf};

use toknila_core::plans::PlanKind;

use super::*;

const TASKS: &str = "# Tasks: Login\n\n## Phase 1: Setup\n- [x] T001 a\n\n## Phase 2: Build\n- [ ] T002 b\n- [ ] T003 c\n";
const CHAIN: &str =
    "[chain]\nname = \"phase-1\"\n\n[[ticket]]\nid = \"A\"\ntitle = \"A\"\nsubtasks = [\"x\"]\n";

/// `root` joined with each part, so the separators are this platform's, as resolved paths have.
fn at(root: &Path, parts: &[&str]) -> PathBuf {
    parts.iter().fold(root.to_owned(), |p, part| p.join(part))
}

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    std::fs::write(path, text).expect("write");
}

/// A repository folder with two spec folders, one folder without tasks, a hidden folder, a
/// chain file, a note that is not one, and a `.specify/` folder.
fn repo() -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::tempdir().expect("tmp");
    let root = canonical(tmp.path()).join("repo");
    write(&root.join("specs/001-login/tasks.md"), TASKS);
    write(&root.join("specs/001-login/spec.md"), "# Login feature\n");
    write(
        &root.join("specs/002-broken/tasks.md"),
        "## Phase 1: A\n- [ ] T1 ok\n- [q] T2 bad\n",
    );
    std::fs::create_dir_all(at(&root, &["specs", "003-notes"])).expect("mkdir");
    write(&root.join("specs/.draft/tasks.md"), TASKS);
    write(&at(&root, &["plans", "phase-1.toml"]), CHAIN);
    write(&at(&root, &["plans", "README.md"]), "not a plan\n");
    std::fs::create_dir_all(root.join(".specify")).expect("mkdir");
    (tmp, root)
}

fn one(found: Found) -> Candidate {
    match found {
        Found::One(c) => c,
        Found::Many { .. } => panic!("one candidate expected"),
    }
}

#[test]
fn a_relative_a_missing_or_an_empty_path_is_refused_by_name() {
    for (path, says) in [
        ("specs/001-login", "specs/001-login is not an absolute path"),
        ("", "give the absolute path"),
    ] {
        let e = read(path).expect_err("refused");
        assert!(e.message.starts_with(says), "{}", e.message);
    }
    let (_tmp, root) = repo();
    let gone = text(&at(&root, &["specs", "404"]));
    let e = read(&gone).expect_err("refused");
    assert_eq!(e.message, format!("{gone} does not exist"));
    let empty = at(&root, &["specs", "003-notes"]);
    let e = read(&text(&empty)).expect_err("refused");
    assert!(e.message.contains("has no tasks.md"), "{}", e.message);
}

#[test]
fn a_spec_folder_is_one_chain_named_after_it_with_its_counts_and_documents() {
    let (_tmp, root) = repo();
    let c = one(read(&text(&at(&root, &["specs", "001-login"]))).expect("reads"));
    assert_eq!(c.folder, "001-login");
    assert_eq!(c.path, text(&at(&root, &["specs", "001-login"])));
    let p = c.chain.expect("a chain file");
    assert_eq!(
        (p.counts.cards, p.counts.tasks, p.counts.done_cards),
        (2, 3, 1)
    );
    assert!(p.toml.contains("name = \"001-login\""), "{}", p.toml);
    assert!(
        p.toml.contains("Documents:\n- spec.md\n- tasks.md"),
        "{}",
        p.toml
    );
}

#[test]
fn a_broken_tasks_file_names_its_file_and_line() {
    let (_tmp, root) = repo();
    let c = one(read(&text(&at(&root, &["specs", "002-broken"]))).expect("reads"));
    let e = c.chain.expect_err("broken");
    let tasks = at(&root, &["specs", "002-broken"]).join("tasks.md");
    assert_eq!(
        e,
        format!(
            "{}:3: malformed task line: expected `- [ ] text` or `- [x] text`",
            tasks.display()
        )
    );
}

#[test]
fn a_chain_file_is_one_chain() {
    let (_tmp, root) = repo();
    let c = one(read(&text(&at(&root, &["plans", "phase-1.toml"]))).expect("reads"));
    assert_eq!(c.folder, "phase-1.toml");
    let p = c.chain.expect("parses");
    assert_eq!((p.counts.cards, p.counts.tasks), (1, 1));
    let bad = at(&root, &["plans", "README.md"]);
    let c = one(read(&text(&bad)).expect("reads"));
    assert!(c.chain.expect_err("not TOML").starts_with(&text(&bad)));
}

#[test]
fn a_folder_of_spec_folders_lists_each_and_skips_the_rest() {
    let (_tmp, root) = repo();
    let Found::Many {
        spec_folders,
        skipped,
    } = read(&text(&root.join("specs"))).expect("reads")
    else {
        panic!("several expected");
    };
    let names: Vec<(&str, bool)> = spec_folders
        .iter()
        .map(|c| (c.folder.as_str(), c.chain.is_ok()))
        .collect();
    assert_eq!(names, [("001-login", true), ("002-broken", false)]);
    let skipped: Vec<(&str, &str)> = skipped
        .iter()
        .map(|s| (s.folder.as_str(), s.reason.as_str()))
        .collect();
    assert_eq!(skipped, [("003-notes", "no tasks.md")]);
}

#[test]
fn the_scan_finds_spec_folders_and_chain_files_one_level_deep() {
    let (_tmp, root) = repo();
    write(&root.join("specs/001-login/nested/tasks.md"), TASKS);
    let sources = vec![text(&at(&root, &["plans", "phase-1.toml"]))];
    let s = scan(Some(&text(&root)), &sources);
    assert!(s.specify_dir);
    let got: Vec<(PlanKind, &str, Option<usize>, bool)> = s
        .plans
        .iter()
        .map(|p| (p.kind, p.relative.as_str(), p.cards, p.imported))
        .collect();
    assert_eq!(
        got,
        [
            (PlanKind::ChainFile, "plans/phase-1.toml", Some(1), true),
            (PlanKind::SpecFolder, "specs/001-login", Some(2), false),
            (PlanKind::SpecFolder, "specs/002-broken", None, false),
        ]
    );
    assert_eq!(s.plans[1].path, text(&at(&root, &["specs", "001-login"])));
}

#[test]
fn no_repository_or_a_missing_one_has_no_plans() {
    assert_eq!(scan(None, &[]), Default::default());
    let tmp = tempfile::tempdir().expect("tmp");
    let gone = text(&tmp.path().join("gone"));
    assert_eq!(scan(Some(&gone), &[]), Default::default());
    assert_eq!(scan(Some("relative/repo"), &[]), Default::default());
}

#[test]
fn a_working_copy_behind_its_upstream_says_so_and_one_that_is_not_says_nothing() {
    use crate::git::testing::{commit_file, git};
    let tmp = tempfile::tempdir().expect("tmp");
    let origin = tmp.path().join("origin");
    std::fs::create_dir_all(&origin).expect("mkdir");
    git(&origin, &["init", "--quiet"]);
    git(&origin, &["symbolic-ref", "HEAD", "refs/heads/main"]);
    commit_file(&origin, "a.txt", "a\n", "a");
    git(tmp.path(), &["clone", "--quiet", &text(&origin), "clone"]);
    let clone = tmp.path().join("clone");
    assert_eq!(scan(Some(&text(&clone)), &[]).behind, None);

    commit_file(&origin, "b.txt", "b\n", "b");
    commit_file(&origin, "c.txt", "c\n", "c");
    git(&clone, &["fetch", "--quiet"]);
    let note = scan(Some(&text(&clone)), &[]).behind.expect("behind");
    assert_eq!(note.note(), "main is 2 commits behind origin/main");
}
