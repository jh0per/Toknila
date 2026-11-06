//! The real runner (a program that is missing, one that answers, one that hangs) and the repo
//! facts against a real temporary repository. The hanging program is this test binary itself,
//! running the `#[ignore]`d test below, so no helper binary or shell is needed.

use std::process::Command;

use super::*;

fn me() -> String {
    std::env::current_exe()
        .expect("exe")
        .to_string_lossy()
        .into_owned()
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

fn git(dir: &std::path::Path, args: &[&str]) {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .output()
        .expect("git runs");
    assert!(out.status.success(), "git {args:?}: {out:?}");
}

#[test]
fn repo_facts_follow_the_disk_and_the_branches() {
    let tmp = tempfile::tempdir().expect("tmp");
    let repo = tmp.path().join("repo");
    std::fs::create_dir_all(&repo).expect("mkdir");
    git(&repo, &["init", "--quiet"]);
    git(&repo, &["symbolic-ref", "HEAD", "refs/heads/develop"]);
    std::fs::write(repo.join("a.txt"), "a").expect("write");
    git(&repo, &["add", "a.txt"]);
    git(&repo, &["commit", "--quiet", "-m", "init"]);
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
    assert_eq!(code(&repo.join("a.txt")), "NotADirectory");
    assert_eq!(code(&tmp.path().join("nope")), "PathMissing");
}
