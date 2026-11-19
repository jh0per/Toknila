//! `toknila daemon start|stop|status|run`.

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use clap::Subcommand;
use serde::Serialize;
use toknila_api_types::{MachineSettings, SoundSettings};
use toknila_client::Client;
use toknila_daemon::info::DaemonInfo;
use toknila_daemon::paths::Paths;

/// Daemon lifecycle.
#[derive(Debug, Subcommand)]
pub enum DaemonCmd {
    /// Start the daemon in the background (no-op if it is already running).
    Start {
        /// Hand the daemon this process's stdin as a lifeline: it stops when that pipe closes.
        #[arg(long)]
        exit_with_parent: bool,
    },
    /// Ask the running daemon to shut down cleanly.
    Stop,
    /// Show whether the daemon is running, and where.
    Status,
    /// Change the daemon's machine settings (without a flag: show them).
    Set {
        /// How many hand-in checks may run at once (default 1). A further one starts only while
        /// the one-minute load average is below the number of CPU cores.
        #[arg(long, value_name = "N")]
        max_checks: Option<u32>,
        /// Play a sound in the window when a story is done: on or off (default off).
        #[arg(long, value_enum, value_name = "on|off")]
        sound_story_done: Option<Switch>,
        /// Play a sound in the window when an epic is finished: on or off (default off).
        #[arg(long, value_enum, value_name = "on|off")]
        sound_epic_done: Option<Switch>,
        /// Play a sound in the window when something went wrong: on or off (default off).
        #[arg(long, value_enum, value_name = "on|off")]
        sound_problem: Option<Switch>,
    },
    /// Run the daemon in the foreground (what `start` launches).
    Run {
        /// Stop (agents included) when stdin, a pipe the parent holds open, closes.
        #[arg(long)]
        exit_with_parent: bool,
    },
}

#[derive(Debug, Serialize)]
struct Status {
    running: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pid: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<String>,
    /// The daemon binary that runs; `None` for a daemon older than 0.2.
    #[serde(skip_serializing_if = "Option::is_none")]
    exe: Option<String>,
    /// Agent slots in use across all projects (working cards and live runners); the desktop app
    /// asks before replacing a daemon that has any. Absent when the daemon does not answer.
    #[serde(skip_serializing_if = "Option::is_none")]
    agents: Option<u32>,
    home: String,
}

/// Agent slots in use on the running daemon, read through its own API (any version has it).
fn agents_in_use(info: &DaemonInfo) -> Option<u32> {
    let all = Client::new(info.base_url(), info.token.clone())
        .agents()
        .ok()?;
    Some(all.iter().map(|p| p.used).sum())
}

pub fn run(cmd: DaemonCmd, json: bool) -> Result<()> {
    let paths = Paths::resolve().map_err(anyhow::Error::msg)?;
    match cmd {
        DaemonCmd::Run { exit_with_parent } => run_foreground(paths, exit_with_parent),
        DaemonCmd::Start { exit_with_parent } => {
            let info = start_with(&paths, exit_with_parent)?;
            print_status(&paths, Some(&info), json);
            Ok(())
        }
        DaemonCmd::Stop => {
            let stopped = stop(&paths)?;
            if json {
                println!("{}", serde_json::json!({ "stopped": stopped }));
            } else if stopped {
                println!("daemon stopped");
            } else {
                println!("daemon is not running");
            }
            Ok(())
        }
        DaemonCmd::Status => {
            let info = running(&paths)?;
            print_status(&paths, info.as_ref(), json);
            Ok(())
        }
        DaemonCmd::Set {
            max_checks,
            sound_story_done,
            sound_epic_done,
            sound_problem,
        } => {
            let sounds = SoundFlags {
                story_done: sound_story_done,
                epic_done: sound_epic_done,
                problem: sound_problem,
            };
            set(&paths, max_checks, sounds, json)
        }
    }
}

/// A switch on the command line.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum Switch {
    On,
    Off,
}

/// The sound switches `daemon set` was asked to change; `None` leaves one as it is.
#[derive(Debug, Clone, Copy, Default)]
pub struct SoundFlags {
    pub story_done: Option<Switch>,
    pub epic_done: Option<Switch>,
    pub problem: Option<Switch>,
}

impl SoundFlags {
    fn any(self) -> bool {
        self.story_done.is_some() || self.epic_done.is_some() || self.problem.is_some()
    }

    fn apply(self, mut s: SoundSettings) -> SoundSettings {
        let on = |flag: Option<Switch>, now: bool| flag.map_or(now, |f| matches!(f, Switch::On));
        s.story_done = on(self.story_done, s.story_done);
        s.epic_done = on(self.epic_done, s.epic_done);
        s.problem = on(self.problem, s.problem);
        s
    }
}

/// `daemon set --max-checks N --sound-story-done on`: the machine settings, through the running
/// daemon.
fn set(paths: &Paths, max_checks: Option<u32>, sounds: SoundFlags, json: bool) -> Result<()> {
    if max_checks == Some(0) {
        bail!("--max-checks must be at least 1");
    }
    let client = crate::client::connect(paths)?;
    let mut settings = client.machine_settings()?;
    if max_checks.is_some() || sounds.any() {
        let body = MachineSettings {
            max_checks: max_checks.unwrap_or(settings.max_checks),
            sounds: Some(sounds.apply(settings.sounds.unwrap_or_default())),
        };
        settings = client.set_machine_settings(&body)?;
    }
    if json {
        println!("{}", serde_json::to_string(&settings)?);
    } else {
        let sounds = settings.sounds.unwrap_or_default();
        let word = |on: bool| if on { "on" } else { "off" };
        println!("checks at once: {}", settings.max_checks);
        println!("sound, a story is done: {}", word(sounds.story_done));
        println!("sound, an epic is finished: {}", word(sounds.epic_done));
        println!("sound, something went wrong: {}", word(sounds.problem));
    }
    Ok(())
}

fn print_status(paths: &Paths, info: Option<&DaemonInfo>, json: bool) {
    let s = Status {
        running: info.is_some(),
        pid: info.map(|i| i.pid),
        port: info.map(|i| i.port),
        version: info.map(|i| i.version.clone()),
        exe: info.and_then(|i| i.exe.clone()),
        agents: info.and_then(agents_in_use),
        home: paths.home.display().to_string(),
    };
    if json {
        println!("{}", serde_json::to_string(&s).unwrap_or_default());
    } else if let Some(i) = info {
        println!(
            "daemon running: pid {} on 127.0.0.1:{} (v{}), home {}",
            i.pid, i.port, i.version, s.home
        );
    } else {
        println!("daemon is not running (home {})", s.home);
    }
}

fn run_foreground(paths: Paths, exit_with_parent: bool) -> Result<()> {
    std::fs::create_dir_all(&paths.home)
        .with_context(|| format!("create {}", paths.home.display()))?;
    // First, while this is the only thread: logging and the runtime start threads.
    let appended = toknila_daemon::adopt_agent_path();
    let _guard = toknila_daemon::server::init_logging(&paths).map_err(anyhow::Error::msg)?;
    toknila_daemon::log_adopted_path(&appended);
    if exit_with_parent {
        toknila_daemon::server::exit_with_parent();
    }
    // The server runs on its own thread with a generous stack: `run` is one large future that
    // startup recovery, the ship retirement and the watchers grow, and Windows gives the main
    // thread one megabyte (the daemon died at startup there with STATUS_STACK_OVERFLOW,
    // 0xc00000fd, on 2026-11-15 while Linux and macOS with their eight were fine). The future
    // itself is boxed, so its state is on the heap whatever its size.
    let server = std::thread::Builder::new()
        .name("daemon".to_owned())
        .stack_size(SERVER_STACK)
        .spawn(move || -> Result<()> {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .thread_stack_size(WORKER_STACK)
                .build()?;
            let result = rt.block_on(Box::pin(toknila_daemon::server::run(paths)));
            // Dropping the runtime would wait for every blocking task (a transcript scan, a git
            // call) to finish, so a stopped daemon would linger; give them a moment, then leave.
            rt.shutdown_timeout(std::time::Duration::from_secs(1));
            result?;
            Ok(())
        })?;
    server
        .join()
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
}

/// The stack of the thread that polls the server's future.
const SERVER_STACK: usize = 16 * 1024 * 1024;
/// The stack of each runtime worker thread (tokio's default is two megabytes).
const WORKER_STACK: usize = 8 * 1024 * 1024;

/// The running daemon's info, or `None` if no daemon answers `/healthz`.
pub fn running(paths: &Paths) -> Result<Option<DaemonInfo>> {
    let Some(info) = DaemonInfo::read_settled(&paths.daemon_json())? else {
        return Ok(None);
    };
    let healthy = Client::healthy(&info.base_url());
    Ok(healthy.then_some(info))
}

/// Starts the daemon if needed and waits until it answers.
pub fn start(paths: &Paths) -> Result<DaemonInfo> {
    start_with(paths, false)
}

/// [`start`]; with `lifeline` the daemon shares this process's stdin and stops when it closes.
pub fn start_with(paths: &Paths, lifeline: bool) -> Result<DaemonInfo> {
    // A daemon that is up but busy (a loaded machine) can miss one health probe; a refused
    // connection answers at once, so a dead daemon's stale file costs only the pauses.
    for attempt in 0..3 {
        if let Some(info) = running(paths)? {
            return Ok(info);
        }
        if attempt < 2 && DaemonInfo::read_settled(&paths.daemon_json())?.is_some() {
            std::thread::sleep(Duration::from_millis(200));
        } else {
            break;
        }
    }
    std::fs::create_dir_all(&paths.home)?;
    let exe = std::env::current_exe().context("cannot find own executable")?;
    let mut cmd = Command::new(exe);
    cmd.args(["daemon", "run"]);
    // The platform default home is not passed: the daemon resolves it itself, with its ships.
    if toknila_daemon::paths::platform_home().as_deref() != Some(paths.home.as_path()) {
        cmd.env(toknila_daemon::paths::HOME_ENV, &paths.home);
    }
    if lifeline {
        cmd.arg("--exit-with-parent").stdin(Stdio::inherit());
    } else {
        cmd.stdin(Stdio::null());
    }
    cmd.stdout(Stdio::null()).stderr(Stdio::null());
    detach(&mut cmd);
    #[cfg(windows)]
    crate::win::stop_inheriting_stdio();
    let mut child = cmd.spawn().context("cannot start the daemon")?;

    let wait = start_wait(
        std::env::var_os("TOKNILA_TEST_ROOT").is_some(),
        std::env::var(START_WAIT_VAR).ok().as_deref(),
    );
    let deadline = Instant::now() + wait;
    while Instant::now() < deadline {
        if let Some(info) = running(paths)? {
            return Ok(info);
        }
        if let Some(status) = child.try_wait()? {
            // Ours lost the race to a daemon that is up: that one is the answer.
            if let Some(info) = running(paths)? {
                return Ok(info);
            }
            bail!(
                "daemon exited during startup ({status}); see logs in {}",
                paths.logs().display()
            );
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    bail!(
        "daemon did not answer within {} s; see logs in {}",
        wait.as_secs(),
        paths.logs().display()
    )
}

/// How long `start` waits for the daemon it started to answer.
const START_WAIT: Duration = Duration::from_secs(15);
/// The seconds a test's `tok` waits instead (see [`start_wait`]).
const START_WAIT_VAR: &str = "TOKNILA_START_WAIT_SECS";

/// [`START_WAIT`], or the seconds in [`START_WAIT_VAR`] when `test_root` (`TOKNILA_TEST_ROOT` is
/// set: a test's `tok`; a person's command ignores the variable). Under the parallel test suite
/// on GitHub's Windows runner a daemon start took up to 11 s (CI run 44308344208) and four tests
/// failed at 15 s (run 44136517698), while one daemon alone answered in 1.5 s.
fn start_wait(test_root: bool, value: Option<&str>) -> Duration {
    if !test_root {
        return START_WAIT;
    }
    value
        .and_then(|v| v.parse::<u64>().ok())
        .filter(|s| *s > 0)
        .map_or(START_WAIT, Duration::from_secs)
}

/// Stops the running daemon. Returns `false` if none was running.
pub fn stop(paths: &Paths) -> Result<bool> {
    let Some(info) = running(paths)? else {
        return Ok(false);
    };
    Client::new(info.base_url(), info.token)
        .shutdown()
        .context("shutdown request failed")?;
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if running(paths)?.is_none() {
            return Ok(true);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    bail!("daemon (pid {}) did not stop within 10 s", info.pid)
}

/// Lets the daemon outlive the terminal that started it.
fn detach(cmd: &mut Command) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_person_waits_fifteen_seconds_whatever_the_variable_says() {
        assert_eq!(start_wait(false, None), Duration::from_secs(15));
        assert_eq!(start_wait(false, Some("60")), Duration::from_secs(15));
    }

    #[test]
    fn a_test_waits_the_seconds_it_was_given_and_fifteen_without_a_usable_value() {
        assert_eq!(start_wait(true, Some("60")), Duration::from_secs(60));
        for value in [None, Some(""), Some("0"), Some("soon"), Some("-5")] {
            assert_eq!(
                start_wait(true, value),
                Duration::from_secs(15),
                "{value:?}"
            );
        }
    }
}
