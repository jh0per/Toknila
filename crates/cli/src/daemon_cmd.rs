//! `toknila daemon start|stop|status|run`.

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use clap::Subcommand;
use serde::Serialize;
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
    }
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
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let result = rt.block_on(toknila_daemon::server::run(paths));
    // Dropping the runtime would wait for every blocking task (a transcript scan, a git call) to
    // finish, so a stopped daemon would linger; give them a moment, then leave.
    rt.shutdown_timeout(std::time::Duration::from_secs(1));
    result?;
    Ok(())
}

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

    let deadline = Instant::now() + Duration::from_secs(15);
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
        "daemon did not answer within 15 s; see logs in {}",
        paths.logs().display()
    )
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
