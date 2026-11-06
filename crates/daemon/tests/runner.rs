//! The PTY runner on a real pseudo-terminal (Unix pty, Windows ConPTY) with the workspace's
//! `fake-agent` as the profile's command: no shell, no OS tools.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::mpsc::UnboundedReceiver;
use toknila_app::runner::{RunEnded, RunRequest, Runner, RunnerError};
use toknila_core::TicketId;
use toknila_daemon::runner::Runners;

const AGENT: &str = env!("CARGO_BIN_EXE_fake-agent");
const WAIT: Duration = Duration::from_secs(20);

struct Rig {
    dir: tempfile::TempDir,
    runners: Arc<Runners>,
    ended: UnboundedReceiver<RunEnded>,
    ticket: TicketId,
}

impl Rig {
    fn new() -> Rig {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(dir.path().join("profiles")).expect("profiles");
        std::fs::create_dir_all(dir.path().join("ship")).expect("ship");
        let (runners, ended) = Runners::new(dir.path().join("runs"), dir.path().join("profiles"));
        Rig {
            dir,
            runners,
            ended,
            ticket: TicketId::new(),
        }
    }

    /// A profile file whose command is the fake agent running `steps`.
    fn profile(&self, name: &str, steps: &[&str], extra: &str) {
        let args: String = steps.iter().map(|s| format!(", '{s}'")).collect();
        let text = format!("command = ['{AGENT}'{args}]\n{extra}");
        std::fs::write(
            self.dir
                .path()
                .join("profiles")
                .join(format!("{name}.toml")),
            text,
        )
        .expect("profile");
    }

    fn ship(&self) -> PathBuf {
        self.dir.path().join("ship")
    }

    fn request(&self, profile: &str) -> RunRequest {
        RunRequest {
            ticket: self.ticket,
            key: "DEMO-1".parse().expect("key"),
            profile: profile.to_owned(),
            dir: self.ship().display().to_string(),
        }
    }

    fn start(&self, profile: &str) -> toknila_app::runner::Started {
        self.runners.start(&self.request(profile)).expect("start")
    }

    /// The output so far, as text.
    fn output(&self) -> String {
        self.runners
            .read(self.ticket, None)
            .map(|c| String::from_utf8_lossy(&c.bytes).into_owned())
            .unwrap_or_default()
    }

    fn wait_for(&self, what: &str) -> String {
        let until = Instant::now() + WAIT;
        loop {
            let out = self.output();
            if out.contains(what) {
                return out;
            }
            assert!(Instant::now() < until, "no {what:?} in {out:?}");
            std::thread::sleep(Duration::from_millis(25));
        }
    }

    fn wait_end(&mut self) -> RunEnded {
        let until = Instant::now() + WAIT;
        loop {
            if let Ok(e) = self.ended.try_recv() {
                return e;
            }
            assert!(
                Instant::now() < until,
                "the process did not end: {:?}",
                self.output()
            );
            std::thread::sleep(Duration::from_millis(25));
        }
    }
}

#[test]
fn output_streams_into_the_scrollback_with_the_ship_as_working_directory() {
    let mut rig = Rig::new();
    rig.profile("fake", &["print", "hello runner", "pwd", "exit", "0"], "");
    let started = rig.start("fake");
    assert!(!started.reused);
    let out = rig.wait_for("ship");
    assert!(out.contains("hello runner"), "{out:?}");
    let end = rig.wait_end();
    assert_eq!(end.run, started.run);
    assert_eq!(end.exit.code, Some(0));
    assert!(!end.stopped);
}

#[test]
fn a_failing_exit_code_is_reported_as_it_is() {
    let mut rig = Rig::new();
    rig.profile("fake", &["print", "boom", "exit", "3"], "");
    rig.start("fake");
    let end = rig.wait_end();
    assert_eq!((end.exit.code, end.exit.signal), (Some(3), None));
    assert!(!end.stopped);
    assert!(
        rig.output().contains("boom"),
        "output is kept after the exit"
    );
}

#[test]
fn input_reaches_the_process_and_its_answer_comes_back() {
    let mut rig = Rig::new();
    rig.profile("fake", &["print", "ready", "echo", "exit", "0"], "");
    rig.start("fake");
    rig.wait_for("ready");
    rig.runners
        .write(rig.ticket, b"ping\r")
        .expect("write input");
    rig.wait_for("got:ping");
    assert_eq!(rig.wait_end().exit.code, Some(0));
}

#[test]
fn stop_kills_a_running_process_and_says_it_was_asked_to() {
    let mut rig = Rig::new();
    rig.profile("fake", &["print", "up", "hold"], "");
    rig.start("fake");
    rig.wait_for("up");
    assert_eq!(rig.runners.live(), vec![rig.ticket]);
    rig.runners.stop(rig.ticket);
    let end = rig.wait_end();
    assert!(end.stopped, "a stop is not a crash");
    assert!(rig.runners.live().is_empty());
    let err = rig
        .runners
        .write(rig.ticket, b"x")
        .expect_err("nobody listens");
    assert_eq!(err, RunnerError::NotRunning);
}

#[test]
fn a_live_runner_is_kept_not_started_twice() {
    let rig = Rig::new();
    rig.profile("fake", &["hold"], "");
    let first = rig.start("fake");
    let second = rig.start("fake");
    assert!(second.reused);
    assert_eq!(first.run.id, second.run.id);
    rig.runners.stop_all();
}

#[test]
fn the_scrollback_keeps_the_last_lines_the_profile_allows_and_the_log_keeps_all() {
    let mut rig = Rig::new();
    let steps: Vec<String> = (1..=30)
        .flat_map(|i| ["print".to_owned(), format!("line-{i:02}")])
        .collect();
    let steps: Vec<&str> = steps.iter().map(String::as_str).collect();
    rig.profile("fake", &steps, "[output]\nscrollback_lines = 5\n");
    let started = rig.start("fake");
    rig.wait_end();
    let kept = rig.output();
    assert!(
        kept.contains("line-30") && kept.contains("line-26"),
        "{kept:?}"
    );
    assert!(
        !kept.contains("line-20"),
        "older lines are dropped: {kept:?}"
    );
    let log = log_of(rig.dir.path(), rig.ticket, &started.run.id.to_string());
    let all = std::fs::read_to_string(log).expect("log");
    assert!(all.contains("line-01") && all.contains("line-30"));
}

#[test]
fn after_a_restart_the_newest_log_is_the_scrollback_and_how_it_ended_is_unknown() {
    let mut rig = Rig::new();
    rig.profile("fake", &["print", "from before", "exit", "0"], "");
    let started = rig.start("fake");
    rig.wait_end();
    let (fresh, _rx) = Runners::new(rig.dir.path().join("runs"), rig.dir.path().join("profiles"));
    let chunk = fresh.read(rig.ticket, None).expect("log on disk");
    assert_eq!(chunk.run, started.run.id);
    assert!(String::from_utf8_lossy(&chunk.bytes).contains("from before"));
    assert!(chunk.ended && chunk.exit.is_none());
    assert!(fresh.read(TicketId::new(), None).is_none());
}

#[test]
fn a_missing_profile_a_sim_profile_and_a_missing_program_are_refused_with_a_typed_error() {
    let rig = Rig::new();
    assert_eq!(
        rig.runners
            .start(&rig.request("nope"))
            .expect_err("unknown"),
        RunnerError::UnknownProfile("nope".to_owned())
    );
    assert_eq!(
        rig.runners.start(&rig.request("sim")).expect_err("sim"),
        RunnerError::NoCommand("sim".to_owned())
    );
    std::fs::write(
        rig.dir.path().join("profiles").join("ghost.toml"),
        "command = ['toknila-no-such-program']\n",
    )
    .expect("profile");
    assert!(matches!(
        rig.runners.start(&rig.request("ghost")),
        Err(RunnerError::Spawn(_))
    ));
}

fn log_of(home: &Path, ticket: TicketId, run: &str) -> PathBuf {
    home.join("runs")
        .join(ticket.to_string())
        .join(format!("{run}.log"))
}

/// The value the fake agent printed for environment variable `name`.
fn printed_env(output: &str, name: &str) -> String {
    output
        .lines()
        .find_map(|l| l.trim().strip_prefix(&format!("{name}=")))
        .unwrap_or_else(|| panic!("no {name} in {output:?}"))
        .trim()
        .to_owned()
}

#[test]
fn each_run_gets_its_own_token_scoped_to_its_ticket_and_revoked_when_it_ends() {
    let mut rig = Rig::new();
    rig.runners
        .set_daemon_url("http://127.0.0.1:4242".to_owned());
    rig.profile(
        "fake",
        &[
            "env",
            "TOKNILA_RUN_TOKEN",
            "env",
            "TOKNILA_TICKET",
            "env",
            "TOKNILA_URL",
            "print",
            "up",
            "hold",
        ],
        "",
    );
    let started = rig.start("fake");
    let out = rig.wait_for("up");
    let token = printed_env(&out, "TOKNILA_RUN_TOKEN");
    assert_eq!(printed_env(&out, "TOKNILA_TICKET"), "DEMO-1");
    assert_eq!(printed_env(&out, "TOKNILA_URL"), "http://127.0.0.1:4242");

    let grant = rig.runners.tokens().lookup(&token).expect("a live token");
    assert_eq!((grant.ticket, grant.run), (rig.ticket, started.run.id));
    assert!(rig.runners.tokens().last_activity(rig.ticket).is_some());

    rig.runners.stop(rig.ticket);
    rig.wait_end();
    assert_eq!(
        rig.runners.tokens().lookup(&token),
        None,
        "revoked once the run ended"
    );
}

#[test]
fn a_run_that_exits_on_its_own_loses_its_token_too_and_the_next_run_has_another() {
    let mut rig = Rig::new();
    rig.profile("fake", &["env", "TOKNILA_RUN_TOKEN", "exit", "0"], "");
    rig.start("fake");
    let first = printed_env(&rig.wait_for("TOKNILA_RUN_TOKEN="), "TOKNILA_RUN_TOKEN");
    rig.wait_end();
    assert_eq!(rig.runners.tokens().lookup(&first), None);

    rig.start("fake");
    let until = Instant::now() + WAIT;
    let second = loop {
        let out = rig.output();
        // The log of the newest run only; the first token's line is gone with its run.
        if let Some(token) = out
            .lines()
            .find_map(|l| l.trim().strip_prefix("TOKNILA_RUN_TOKEN="))
            .filter(|t| t.trim().len() == 64)
        {
            break token.trim().to_owned();
        }
        assert!(Instant::now() < until, "no second token in {out:?}");
        std::thread::sleep(Duration::from_millis(25));
    };
    assert_ne!(first, second);
    rig.wait_end();
}

#[test]
fn a_profile_that_asks_for_mcp_gets_a_config_file_under_the_data_home_and_the_flag() {
    let mut rig = Rig::new();
    // The fake agent has no `--mcp-config`; its `print` step stands in for the flag and prints
    // the path it is given.
    rig.profile(
        "fake",
        &[],
        "[mcp]\nenabled = true\nconfig_flag = \"print\"\n",
    );
    rig.start("fake");
    rig.wait_for("mcp.json");
    rig.wait_end();
    let file = rig
        .dir
        .path()
        .join("runs")
        .join(rig.ticket.to_string())
        .join("mcp.json");
    let text = std::fs::read_to_string(&file).expect("generated config");
    assert!(text.contains("\"mcpServers\""), "{text}");
    assert!(
        !rig.ship().join("mcp.json").exists(),
        "nothing is written into the worktree"
    );
}
