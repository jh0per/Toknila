//! The PTY runner on a real pseudo-terminal (Unix pty, Windows ConPTY) with the workspace's
//! `fake-agent` as the profile's command: no shell, no OS tools.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::mpsc::UnboundedReceiver;
use toknila_app::runner::{RunEnded, RunRequest, Runner, RunnerError};
use toknila_core::{AgentRole, TicketId};
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
        self.request_as(profile, AgentRole::Worker)
    }

    fn request_as(&self, profile: &str, role: AgentRole) -> RunRequest {
        RunRequest {
            ticket: self.ticket,
            key: "DEMO-1".parse().expect("key"),
            role,
            profile: profile.to_owned(),
            dir: self.ship().display().to_string(),
            brief: Default::default(),
        }
    }

    fn start(&self, profile: &str) -> toknila_app::runner::Started {
        self.runners.start(&self.request(profile)).expect("start")
    }

    /// The output so far of the worker, as text.
    fn output(&self) -> String {
        self.output_of(AgentRole::Worker)
    }

    fn output_of(&self, role: AgentRole) -> String {
        self.runners
            .read(self.ticket, role, None)
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

/// A process that prints and exits at once must not lose its output (on macOS the kernel drops
/// what is undelivered when the last slave end closes): the reader has to have it before the end
/// is reported.
#[test]
fn the_output_of_a_process_that_prints_and_exits_at_once_is_never_lost() {
    // Several at once: the loss needs the reader to be scheduled late.
    std::thread::scope(|s| {
        for lane in 0..8 {
            s.spawn(move || {
                for i in 0..10 {
                    let mut rig = Rig::new();
                    rig.profile("fake", &["print", "quick words", "exit", "0"], "");
                    let started = rig.start("fake");
                    rig.wait_end();
                    let kept = rig.output();
                    assert!(kept.contains("quick words"), "{lane}/{i}: {kept:?}");
                    assert!(!kept.contains("toknila-drained"), "mark leaked: {kept:?}");
                    let log = log_of(rig.dir.path(), rig.ticket, &started.run.id.to_string());
                    let on_disk = std::fs::read_to_string(log).expect("log");
                    assert!(
                        on_disk.contains("quick words"),
                        "{lane}/{i}: log {on_disk:?}"
                    );
                }
            });
        }
    });
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
        .write(rig.ticket, AgentRole::Worker, b"ping\r")
        .expect("write input");
    rig.wait_for("got:ping");
    assert_eq!(rig.wait_end().exit.code, Some(0));
}

#[test]
fn the_terminal_starts_at_the_default_size_and_follows_a_resize() {
    use toknila_core::instructions::terminal::{self, Size};
    let mut rig = Rig::new();
    rig.profile(
        "fake",
        &["print", "ready", "size", "echo", "size", "exit", "0"],
        "",
    );
    rig.start("fake");
    let first = rig.wait_for("size=");
    let d = terminal::DEFAULT;
    assert!(
        first.contains(&format!("size={}x{}", d.cols, d.rows)),
        "{first:?}"
    );
    rig.runners
        .resize(rig.ticket, Size { cols: 90, rows: 30 })
        .expect("resize");
    rig.runners
        .write(rig.ticket, AgentRole::Worker, b"go\r")
        .expect("write");
    rig.wait_for("size=90x30");
    assert_eq!(rig.wait_end().exit.code, Some(0));
    let err = rig
        .runners
        .resize(rig.ticket, Size { cols: 90, rows: 30 })
        .expect_err("over");
    assert_eq!(err, RunnerError::NotRunning);
}

#[test]
fn a_run_is_idle_after_a_quiet_while_and_reports_whether_it_has_hooks() {
    let rig = Rig::new();
    rig.profile("fake", &["print", "up", "hold"], "");
    rig.start("fake");
    rig.wait_for("up");
    let state = rig.runners.state(rig.ticket).expect("live");
    assert!(!state.hooks, "the profile wires no hooks");
    assert!(!state.idle, "it has just printed");
    let until = Instant::now() + WAIT;
    while !rig.runners.state(rig.ticket).is_some_and(|s| s.idle) {
        assert!(Instant::now() < until, "never idle");
        std::thread::sleep(Duration::from_millis(100));
    }
    rig.runners.stop_all();
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
        .write(rig.ticket, AgentRole::Worker, b"x")
        .expect_err("nobody listens");
    assert_eq!(err, RunnerError::NotRunning);
}

#[test]
fn a_reviewer_runs_next_to_the_worker_with_its_own_output_token_and_stop() {
    let mut rig = Rig::new();
    rig.profile(
        "fake",
        &["print", "worker up", "echo", "print", "worker done", "hold"],
        "",
    );
    rig.profile(
        "fake-reviewer",
        &[
            "env",
            "TOKNILA_ROLE",
            "env",
            "TOKNILA_RUN_TOKEN",
            "print",
            "reviewer up",
            "hold",
        ],
        "",
    );
    let worker = rig.start("fake");
    let reviewer = rig
        .runners
        .start(&rig.request_as("fake-reviewer", AgentRole::Reviewer))
        .expect("start the reviewer");
    assert!(
        !reviewer.reused,
        "the live worker did not count as a reviewer"
    );
    assert_ne!(worker.run.id, reviewer.run.id);
    assert_eq!(reviewer.run.role, AgentRole::Reviewer);
    assert_eq!(worker.run.role, AgentRole::Worker);

    // Each role has its own terminal: the ticket's output is still the worker's.
    let rev_out = {
        let until = Instant::now() + WAIT;
        loop {
            let out = rig.output_of(AgentRole::Reviewer);
            if out.contains("reviewer up") {
                break out;
            }
            assert!(Instant::now() < until, "no reviewer output in {out:?}");
            std::thread::sleep(Duration::from_millis(25));
        }
    };
    assert!(rev_out.contains("TOKNILA_ROLE=reviewer"), "{rev_out:?}");
    let out = rig.wait_for("worker up");
    assert!(!out.contains("reviewer up"), "{out:?}");

    // Its token is a reviewer's; the worker's is the worker's.
    let token = printed_env(&rev_out, "TOKNILA_RUN_TOKEN");
    let grant = rig.runners.tokens().lookup(&token).expect("a live token");
    assert_eq!(
        (grant.ticket, grant.run, grant.role),
        (rig.ticket, reviewer.run.id, AgentRole::Reviewer)
    );
    assert_eq!(rig.runners.live_in(AgentRole::Reviewer), vec![rig.ticket]);
    assert_eq!(rig.runners.live(), vec![rig.ticket]);

    // Typing goes to the role it is addressed to.
    rig.runners
        .write(rig.ticket, AgentRole::Worker, b"hello\r")
        .expect("write");
    rig.wait_for("worker done");
    assert!(!rig.output_of(AgentRole::Reviewer).contains("got:hello"));

    // Stopping the reviewer leaves the worker alone, and its token dies with it.
    rig.runners.stop_role(rig.ticket, AgentRole::Reviewer);
    let end = rig.wait_end();
    assert_eq!((end.run.role, end.stopped), (AgentRole::Reviewer, true));
    assert_eq!(rig.runners.tokens().lookup(&token), None);
    assert!(rig.runners.live_in(AgentRole::Reviewer).is_empty());
    assert_eq!(rig.runners.live(), vec![rig.ticket], "the worker lives on");

    // A stop of the whole ticket ends what is left.
    rig.runners.stop(rig.ticket);
    let end = rig.wait_end();
    assert_eq!((end.run.role, end.stopped), (AgentRole::Worker, true));
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
    let chunk = fresh
        .read(rig.ticket, AgentRole::Worker, None)
        .expect("log on disk");
    assert_eq!(chunk.run, started.run.id);
    assert!(String::from_utf8_lossy(&chunk.bytes).contains("from before"));
    assert!(chunk.ended && chunk.exit.is_none());
    assert!(
        fresh
            .read(TicketId::new(), AgentRole::Worker, None)
            .is_none()
    );
    assert!(
        fresh.read(rig.ticket, AgentRole::Reviewer, None).is_none(),
        "the worker's log is not the reviewer's"
    );
}

#[test]
fn a_log_that_passes_its_cap_rotates_and_the_scrollback_and_offsets_stay_whole() {
    let mut rig = Rig::new();
    let steps: Vec<String> = (1..=200)
        .flat_map(|i| {
            [
                "print".to_owned(),
                format!("line-{i:03}-{}", "x".repeat(40)),
            ]
        })
        .collect();
    let steps: Vec<&str> = steps.iter().map(String::as_str).collect();
    rig.profile("fake", &steps, "[output]\nmax_log_bytes = 2000\n");
    let started = rig.start("fake");
    rig.wait_end();
    let live = rig
        .runners
        .read(rig.ticket, AgentRole::Worker, None)
        .expect("live");
    assert!(
        live.bytes.windows(8).any(|w| w == b"line-200"),
        "the tail is served"
    );
    assert_eq!(
        live.offset, 0,
        "the scrollback holds the whole run: its offsets are the run's"
    );
    assert_eq!(live.end, live.bytes.len() as u64);
    let log = log_of(rig.dir.path(), rig.ticket, &started.run.id.to_string());
    let rotated = PathBuf::from(format!("{}.1", log.display()));
    let (now, before) = (
        std::fs::read(&log).expect("log"),
        std::fs::read(&rotated).expect("rotated log"),
    );
    assert!(
        now.len() <= 2000 && before.len() <= 2000,
        "{} {}",
        now.len(),
        before.len()
    );
    assert!(now.windows(8).any(|w| w == b"line-200"));
    // A restarted daemon sees the same end offset and the same tail, from the two files.
    let (fresh, _rx) = Runners::new(rig.dir.path().join("runs"), rig.dir.path().join("profiles"));
    let after = fresh
        .read(rig.ticket, AgentRole::Worker, None)
        .expect("log on disk");
    assert_eq!(after.end, live.end);
    assert!(after.bytes.windows(8).any(|w| w == b"line-200"));
    assert_eq!(
        after.offset + after.bytes.len() as u64,
        after.end,
        "the offset of the tail matches the run's offsets"
    );
    assert_eq!(&live.bytes[(after.offset as usize)..], &after.bytes[..]);
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

/// The value the fake agent printed for environment variable `name`, if it did yet. ConPTY puts
/// terminal escapes before the first line, so the line is searched for, not matched from its start.
fn find_env(output: &str, name: &str) -> Option<String> {
    let at = output.find(&format!("{name}="))? + name.len() + 1;
    let value: String = output[at..]
        .chars()
        .take_while(|c| !c.is_whitespace() && !c.is_control())
        .collect();
    // The value is complete once its line ended.
    output[at + value.len()..]
        .starts_with(['\r', '\n'])
        .then_some(value)
}

fn printed_env(output: &str, name: &str) -> String {
    find_env(output, name).unwrap_or_else(|| panic!("no {name} in {output:?}"))
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
    rig.wait_for("TOKNILA_RUN_TOKEN=");
    let first = printed_env(&rig.wait_for("\n"), "TOKNILA_RUN_TOKEN");
    rig.wait_end();
    assert_eq!(rig.runners.tokens().lookup(&first), None);

    rig.start("fake");
    let until = Instant::now() + WAIT;
    let second = loop {
        let out = rig.output();
        if let Some(token) = find_env(&out, "TOKNILA_RUN_TOKEN").filter(|t| t.len() == 64) {
            break token;
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
        .join("worker")
        .join("mcp.json");
    let text = std::fs::read_to_string(&file).expect("generated config");
    assert!(text.contains("\"mcpServers\""), "{text}");
    assert!(
        !rig.ship().join("mcp.json").exists(),
        "nothing is written into the worktree"
    );
}

/// Whether a process with this id is running (a zombie is not).
fn alive(pid: u32) -> bool {
    use sysinfo::{Pid, ProcessStatus, ProcessesToUpdate, System};
    let mut sys = System::new().expect("process listing");
    let pid = Pid::from_u32(pid);
    sys.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
    sys.process(pid)
        .is_some_and(|p| !matches!(p.status(), ProcessStatus::Zombie | ProcessStatus::Dead))
}

/// The process id the fake agent printed as `child=<pid>`.
fn child_pid(output: &str) -> u32 {
    let after = output.split("child=").nth(1).unwrap_or("");
    after
        .split_whitespace()
        .next()
        .unwrap_or("")
        .parse()
        .expect("pid")
}

fn wait_until(what: &str, mut done: impl FnMut() -> bool) {
    let until = Instant::now() + WAIT;
    while !done() {
        assert!(Instant::now() < until, "{what}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn a_live_child_of_the_agent_is_seen_and_an_agent_without_one_is_not() {
    let rig = Rig::new();
    rig.profile("fake", &["print", "up", "hold"], "");
    rig.start("fake");
    rig.wait_for("up");
    let state = rig.runners.state(rig.ticket).expect("live");
    assert!(!state.busy, "no child yet");
    rig.runners.stop_all();

    let rig = Rig::new();
    rig.profile("fake", &["child", "hold"], "");
    rig.start("fake");
    let out = rig.wait_for("child=");
    let child = child_pid(&out);
    wait_until("the child is never seen", || {
        rig.runners.state(rig.ticket).is_some_and(|s| s.busy)
    });
    rig.runners.stop_all();
    wait_until("the child outlived the stop", || !alive(child));
}

#[test]
fn stop_ends_the_whole_process_tree() {
    let mut rig = Rig::new();
    rig.profile("fake", &["child", "hold"], "");
    rig.start("fake");
    let out = rig.wait_for("child=");
    let child = child_pid(&out);
    assert!(alive(child), "the child runs before the stop");
    rig.runners.stop(rig.ticket);
    assert!(rig.wait_end().stopped);
    wait_until("the agent's child survived the stop", || !alive(child));
}

#[test]
fn stopping_a_reviewer_ends_its_process_tree_too() {
    let mut rig = Rig::new();
    rig.profile("fake-reviewer", &["child", "hold"], "");
    rig.runners
        .start(&rig.request_as("fake-reviewer", AgentRole::Reviewer))
        .expect("start the reviewer");
    let until = Instant::now() + WAIT;
    let child = loop {
        let out = rig.output_of(AgentRole::Reviewer);
        if out.contains("child=") {
            break child_pid(&out);
        }
        assert!(Instant::now() < until, "no child in {out:?}");
        std::thread::sleep(Duration::from_millis(25));
    };
    assert!(alive(child), "the child runs before the stop");
    rig.runners.stop_role(rig.ticket, AgentRole::Reviewer);
    assert!(rig.wait_end().stopped);
    wait_until("the reviewer's child survived the stop", || !alive(child));
}

/// The marker the outer run sets for the inner one: the real body runs only there, with a daemon
/// environment that has the variables this test is about. (A test cannot change its own
/// environment: that is `unsafe`, denied here.)
const INNER: &str = "TOKNILA_TEST_INNER";

#[test]
fn claude_code_variables_of_the_daemon_do_not_reach_the_agent_unless_its_profile_sets_them() {
    if std::env::var_os(INNER).is_none() {
        let out = std::process::Command::new(std::env::current_exe().expect("test binary"))
            .args([
                "--exact",
                "claude_code_variables_of_the_daemon_do_not_reach_the_agent_unless_its_profile_sets_them",
                "--nocapture",
            ])
            .env(INNER, "1")
            .env("CLAUDECODE", "1")
            .env("CLAUDE_CODE_ENTRYPOINT", "cli")
            .env("CLAUDE_CODE_MINE", "from-the-daemon")
            .output()
            .expect("run the inner test");
        assert!(
            out.status.success(),
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        return;
    }
    // Controls: the daemon really has them.
    assert_eq!(std::env::var("CLAUDECODE").as_deref(), Ok("1"));
    assert_eq!(
        std::env::var("CLAUDE_CODE_ENTRYPOINT").as_deref(),
        Ok("cli")
    );

    let mut rig = Rig::new();
    rig.profile(
        "fake",
        &[
            "env",
            "CLAUDECODE",
            "env",
            "CLAUDE_CODE_ENTRYPOINT",
            "env",
            "CLAUDE_CODE_MINE",
            "env",
            "TOKNILA_TEST_INNER",
            "exit",
            "0",
        ],
        "[env]\nCLAUDE_CODE_MINE = \"from-the-profile\"\n",
    );
    rig.start("fake");
    let out = rig.wait_for("TOKNILA_TEST_INNER=1");
    assert_eq!(printed_env(&out, "CLAUDECODE"), "", "dropped: {out:?}");
    assert_eq!(
        printed_env(&out, "CLAUDE_CODE_ENTRYPOINT"),
        "",
        "dropped: {out:?}"
    );
    assert_eq!(
        printed_env(&out, "CLAUDE_CODE_MINE"),
        "from-the-profile",
        "the profile's own value wins: {out:?}"
    );
    assert_eq!(
        printed_env(&out, "TOKNILA_TEST_INNER"),
        "1",
        "other variables are inherited as before"
    );
    rig.wait_end();
}

#[test]
fn typing_addressed_to_the_reviewer_reaches_the_reviewer_and_not_the_worker() {
    let rig = Rig::new();
    rig.profile("fake", &["print", "ready", "echo", "hold"], "");
    rig.start("fake");
    rig.wait_for("ready");
    rig.runners
        .start(&rig.request_as("fake", AgentRole::Reviewer))
        .expect("start reviewer");
    let until = Instant::now() + WAIT;
    while !rig.output_of(AgentRole::Reviewer).contains("ready") {
        assert!(Instant::now() < until, "the reviewer never started");
        std::thread::sleep(Duration::from_millis(25));
    }
    rig.runners
        .write(rig.ticket, AgentRole::Reviewer, b"hi\r")
        .expect("write to the reviewer");
    while !rig.output_of(AgentRole::Reviewer).contains("got:hi") {
        assert!(Instant::now() < until, "the reviewer never got the text");
        std::thread::sleep(Duration::from_millis(25));
    }
    assert!(
        !rig.output().contains("got:hi"),
        "the worker did not get it"
    );
    rig.runners.stop(rig.ticket);
}

#[test]
fn a_down_arrow_chooses_the_unnumbered_yes_of_a_trust_dialog_and_enter_alone_declines() {
    use toknila_core::question::{self, AnswerKind, Delivery};
    let keys = |answer| match question::deliver(answer, None, false) {
        Ok(Delivery::Keys(bytes)) => bytes,
        other => panic!("keys expected: {other:?}"),
    };
    for (name, sent, expect) in [
        (
            "down-then-enter",
            [keys(AnswerKind::Down), keys(AnswerKind::Enter)].concat(),
            "trust=yes",
        ),
        ("enter-only", keys(AnswerKind::Enter), "trust=no"),
    ] {
        let rig = Rig::new();
        rig.profile("fake", &["trust", "hold"], "");
        rig.start("fake");
        rig.wait_for("Yes, I trust this folder");
        // The screen the answer is given to is hashed before the keys change it.
        let before =
            rig.runners
                .screen_answer(rig.ticket, AgentRole::Worker, toknila_core::Timestamp(1));
        rig.runners
            .write(rig.ticket, AgentRole::Worker, &sent)
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        let out = rig.wait_for("trust=");
        assert!(out.contains(expect), "{name}: {out:?}");
        let after =
            rig.runners
                .screen_answer(rig.ticket, AgentRole::Worker, toknila_core::Timestamp(2));
        assert_ne!(before.screen, after.screen, "{name}: the screen changed");
        rig.runners.stop(rig.ticket);
    }
}
