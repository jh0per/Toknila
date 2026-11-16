//! A daemon under a test root (`TOKNILA_TEST_ROOT`) refuses to launch a real agent program: the
//! start fails before any process, with the sentence the card blocks with.

use toknila_app::runner::{RunRequest, Runner, RunnerError};
use toknila_core::{AgentRole, TicketId};
use toknila_daemon::runner::{REAL_AGENT_REFUSAL, Runners};

fn request(dir: &std::path::Path, profile: &str) -> RunRequest {
    RunRequest {
        ticket: TicketId::new(),
        key: "DEMO-1".parse().expect("key"),
        role: AgentRole::Worker,
        profile: profile.to_owned(),
        dir: dir.display().to_string(),
        brief: Default::default(),
    }
}

#[test]
fn a_test_daemon_blocks_the_default_profile_and_spawns_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(dir.path().join("profiles")).expect("profiles");
    let (runners, mut ended) = Runners::new(dir.path().join("runs"), dir.path().join("profiles"));
    runners.set_refuse_real_agent(true);
    // Every built-in profile that names a program (all of them run `claude`).
    for name in ["claude-code", "claude-code-reviewer", "claude-code-planner"] {
        let err = runners
            .start(&request(dir.path(), name))
            .expect_err("a real agent must not start under a test root");
        assert!(
            err.to_string().contains(REAL_AGENT_REFUSAL),
            "{name}: {err}"
        );
    }
    assert_eq!(
        REAL_AGENT_REFUSAL,
        "A test may not start a real agent: give the project --sim or a fake --profile"
    );
    // Nothing was launched: no live run, no run record (log folder), no process end.
    assert!(runners.live_runners().is_empty());
    assert!(!dir.path().join("runs").exists(), "a run record was made");
    assert!(ended.try_recv().is_err());
}

#[test]
fn the_refusal_is_not_a_blanket_a_profile_file_passes_the_guard() {
    // Control: a profile a test wrote is not refused (here its program does not exist, so the start
    // fails later, with another reason than the refusal; nothing real is ever started).
    let dir = tempfile::tempdir().expect("tempdir");
    let profiles = dir.path().join("profiles");
    std::fs::create_dir_all(&profiles).expect("profiles");
    std::fs::write(
        profiles.join("fake.toml"),
        "command = ['/nonexistent/toknila-not-a-program']\n",
    )
    .expect("profile");
    let (runners, _ended) = Runners::new(dir.path().join("runs"), profiles);
    runners.set_refuse_real_agent(true);
    match runners.start(&request(dir.path(), "fake")) {
        Err(RunnerError::Spawn(m) | RunnerError::NoPty(m)) => {
            assert!(!m.contains(REAL_AGENT_REFUSAL), "{m}");
        }
        other => panic!("expected a spawn failure, got {other:?}"),
    }
}
