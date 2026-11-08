//! The real runner (a program that is missing, one that answers, one that hangs) and the repo
//! facts against a real temporary repository. The hanging program is this test binary itself,
//! running the `#[ignore]`d test below, so no helper binary or shell is needed.

use super::*;
use crate::git::testing::{git, init_repo};

fn me() -> String {
    std::env::current_exe()
        .expect("exe")
        .to_string_lossy()
        .into_owned()
}

#[test]
fn a_check_command_whose_program_is_not_on_the_agents_path_is_reported() {
    let commands = vec!["no-such-program-p208 --all".to_owned()];
    let outside = tempfile::tempdir().expect("tmp");
    let checks = system_checks_for(&SystemTools, &commands, outside.path()).checks;
    let line = checks
        .iter()
        .find(|c| c.tool == system::Tool::CheckCommand)
        .expect("a check command line");
    assert_eq!(line.status, system::CheckStatus::Missing);
    assert!(line.detail.contains("`no-such-program-p208`"), "{line:?}");
    assert!(
        system_checks_for(&SystemTools, &[], outside.path())
            .checks
            .iter()
            .all(|c| c.tool != system::Tool::CheckCommand)
    );
}

#[test]
fn a_data_home_inside_a_repository_warns_and_one_outside_does_not() {
    let tmp = tempfile::tempdir().expect("tmp");
    let repo = tmp.path().join("repo");
    init_repo(&repo, "main");
    let inside = repo.join(".toknila");
    std::fs::create_dir_all(&inside).expect("mkdir");
    let warn = |home: &Path| {
        system_checks_for(&SystemTools, &[], home)
            .checks
            .into_iter()
            .find(|c| c.tool == system::Tool::DataHome)
    };
    let line = warn(&inside).expect("a data home warning");
    assert_eq!(line.status, system::CheckStatus::Warning);
    assert!(
        line.detail.contains("inside the git repository"),
        "{line:?}"
    );
    let plain = tempfile::tempdir().expect("tmp");
    assert_eq!(warn(plain.path()), None);
}

#[test]
fn a_program_that_is_not_installed_is_missing() {
    let probe = SystemTools.run("toknila-no-such-program", &[], Duration::from_secs(5));
    assert_eq!(probe, Probe::Missing);
}

#[test]
fn a_program_that_answers_returns_its_output_and_code() {
    let probe = SystemTools.run(
        &me(),
        &["--list", "--format", "terse"],
        Duration::from_secs(30),
    );
    let Probe::Ran { code, stdout, .. } = probe else {
        panic!("did not run: {probe:?}");
    };
    assert_eq!(code, Some(0));
    assert!(
        stdout.contains("tools::tests::a_program_that_answers"),
        "{stdout}"
    );
}

/// Only runs when the test above starts it as a child: hangs for a minute.
#[test]
#[ignore = "started as a child by a_program_that_hangs_is_stopped_at_the_timeout"]
fn hang_for_a_minute() {
    std::thread::sleep(Duration::from_secs(60));
}

#[test]
fn a_program_that_hangs_is_stopped_at_the_timeout() {
    let started = Instant::now();
    let args = ["--ignored", "--exact", "tools::tests::hang_for_a_minute"];
    let probe = SystemTools.run(&me(), &args, Duration::from_millis(500));
    assert_eq!(probe, Probe::TimedOut);
    assert!(
        started.elapsed() < Duration::from_secs(20),
        "{:?}",
        started.elapsed()
    );
}

#[test]
fn repo_facts_follow_the_disk_and_the_branches() {
    let tmp = tempfile::tempdir().expect("tmp");
    let repo = tmp.path().join("repo");
    init_repo(&repo, "develop");
    git(&repo, &["branch", "release/1"]);
    let path = repo.to_string_lossy().into_owned();

    let ok = check_repo(&SystemTools, &path, "develop");
    assert!(ok.is_ok(), "{ok:?}");
    assert_eq!(ok.branches, ["develop", "release/1"]);

    let missing = check_repo(&SystemTools, &path, "main");
    assert_eq!(missing.problems.len(), 1, "{missing:?}");
    assert_eq!(missing.branches, ["develop", "release/1"]);

    let plain = tmp.path().join("plain");
    std::fs::create_dir_all(&plain).expect("mkdir");
    let code = |p: &std::path::Path| {
        let c = check_repo(&SystemTools, &p.to_string_lossy(), "develop");
        format!("{:?}", c.problems[0].code)
    };
    assert_eq!(code(&plain), "NotAGitRepo");
    assert_eq!(code(&repo.join("README.md")), "NotADirectory");
    assert_eq!(code(&tmp.path().join("nope")), "PathMissing");
}
