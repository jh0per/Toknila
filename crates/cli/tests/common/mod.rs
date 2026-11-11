//! Helpers shared by the end-to-end tests: the real `tok` binary against a temporary data
//! folder, and JSON snapshots in `tests/snapshots/` (`TOKNILA_UPDATE_SNAPSHOTS=1` rewrites them
//! after an intended change).

// Each test file uses only some helpers.
#![allow(dead_code)]

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::{Mutex, Once, PoisonError};
use std::time::{Duration, Instant};

use serde_json::Value;
use toknila_client::Client;
use toknila_daemon::info::DaemonInfo;

/// The `tok` binary of this build, run against one data folder.
pub struct Tok {
    pub home: PathBuf,
}

impl Tok {
    /// A `tok` whose data folder is `<dir>/home`.
    pub fn in_dir(dir: &Path) -> Self {
        let home = dir.join("home");
        track(&home);
        Self { home }
    }

    /// `tok <args>` against this data folder, not yet run.
    fn command(&self, args: &[&str]) -> Command {
        // The daemon of a test is started with a lifeline, so it dies with this process.
        if args.first() != Some(&"daemon") && !daemon_healthy(&self.home) {
            start_daemon(&self.home);
        }
        let mut c = Command::new(env!("CARGO_BIN_EXE_tok"));
        c.args(args).env("TOKNILA_HOME", &self.home);
        isolate(&mut c, &self.home);
        c
    }

    /// Runs `tok <args>` and returns the whole outcome, whether it succeeded or not.
    pub fn raw(&self, args: &[&str]) -> Output {
        self.command(args).output().expect("run tok")
    }

    /// Runs `tok <args> --json` and parses what it prints.
    pub fn run(&self, args: &[&str]) -> Value {
        let out = self.command(args).arg("--json").output().expect("run tok");
        assert!(
            out.status.success(),
            "tok {args:?}: {}\n{}",
            out.status,
            String::from_utf8_lossy(&out.stderr)
        );
        if out.stdout.is_empty() {
            return Value::Null;
        }
        serde_json::from_slice(&out.stdout).unwrap_or_else(|e| {
            panic!(
                "tok {args:?} printed non-JSON ({e}): {}",
                String::from_utf8_lossy(&out.stdout)
            )
        })
    }

    /// Runs `tok <args>` without `--json`, expects it to succeed and returns what it printed.
    pub fn text(&self, args: &[&str]) -> String {
        let out = self.command(args).output().expect("run tok");
        assert!(
            out.status.success(),
            "tok {args:?}: {}\n{}",
            out.status,
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n")
    }

    /// Runs `tok <args>`, expects it to fail and returns its stderr.
    pub fn fails(&self, args: &[&str]) -> String {
        let out = self.command(args).output().expect("run tok");
        assert!(!out.status.success(), "tok {args:?} should fail");
        String::from_utf8_lossy(&out.stderr).into_owned()
    }

    /// The status of one ticket.
    pub fn status(&self, key: &str) -> String {
        self.run(&["show", key])["ticket"]["status"]
            .as_str()
            .expect("status")
            .to_owned()
    }

    /// Waits until a card handed in is past its Check turn: the daemon brings a ship up to its base
    /// and runs the check command after the hand-in answers, and a Return or an Accept is refused
    /// meanwhile. Returns when the review waits for a person (or for anyone else), or the card is
    /// no longer in Review.
    pub fn wait_for_person_turn(&self, key: &str) {
        let until = Instant::now() + Duration::from_secs(120);
        loop {
            let card = self.run(&["show", key]);
            let by = &card["review"]["awaiting"]["by"];
            if card["ticket"]["status"] != "review" || (!by.is_null() && by != "check") {
                return;
            }
            assert!(
                Instant::now() < until,
                "timed out waiting for {key} to wait for a person: {}",
                card["review"]
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// `KEY status` of every ticket, in key order.
    pub fn statuses(&self) -> Vec<String> {
        self.run(&["ls"])
            .as_array()
            .expect("ls is a list")
            .iter()
            .map(|t| {
                format!(
                    "{} {}",
                    t["ticket"]["key"].as_str().unwrap_or("?"),
                    t["ticket"]["status"].as_str().unwrap_or("?")
                )
            })
            .collect()
    }

    pub fn chain(&self) -> Value {
        self.run(&["chain", "show", "demo"])
    }

    /// What `tok logs <ticket>` prints; empty when it cannot (yet).
    pub fn logs(&self, ticket: &str) -> String {
        let out = self
            .command(&["logs", ticket])
            .output()
            .expect("run tok logs");
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    /// `<home>/profiles/<name>.toml` running the fake agent with `steps`; `extra` is more TOML.
    pub fn profile(&self, name: &str, steps: &[&str], extra: &str) {
        let args: String = steps.iter().map(|s| format!(", '{s}'")).collect();
        let folder = self.home.join("profiles");
        std::fs::create_dir_all(&folder).expect("mkdir");
        let text = format!("command = ['{}'{args}]\n{extra}", fake_agent().display());
        std::fs::write(folder.join(format!("{name}.toml")), text).expect("profile");
    }

    /// The address and token the running daemon wrote to `daemon.json`.
    pub fn info(&self) -> DaemonInfo {
        DaemonInfo::read(&self.home.join("daemon.json"))
            .expect("daemon.json")
            .expect("daemon runs")
    }

    /// A client of the running daemon.
    pub fn client(&self) -> Client {
        let info = self.info();
        Client::new(info.base_url(), info.token)
    }
}

/// One `tok mcp` session against the daemon at `url`: the `lines` are sent, stdin is closed, and
/// every answer is returned. `identity` is the environment an agent's session would have
/// (`TOKNILA_TICKET`, `TOKNILA_ROLE`); the other identity variables are removed, since the test
/// may itself run inside an agent's session.
pub fn mcp_session(
    url: &str,
    token: &str,
    identity: &[(&str, &str)],
    lines: &[Value],
) -> Vec<Value> {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_tok"));
    cmd.arg("mcp")
        .env("TOKNILA_URL", url)
        .env("TOKNILA_RUN_TOKEN", token)
        // Nothing of the data home is needed: the daemon is found through the environment.
        .env("TOKNILA_HOME", "/nonexistent-toknila-home");
    for var in ["TOKNILA_TICKET", "TOKNILA_ROLE", "TOKNILA_RUN"] {
        match identity.iter().find(|(k, _)| *k == var) {
            Some((_, v)) => cmd.env(var, v),
            None => cmd.env_remove(var),
        };
    }
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn tok mcp");
    {
        let mut stdin = child.stdin.take().expect("stdin");
        for line in lines {
            writeln!(stdin, "{line}").expect("write");
        }
    }
    let out = child.wait_with_output().expect("wait");
    assert!(
        out.status.success(),
        "tok mcp failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout)
        .expect("utf-8")
        .lines()
        .map(|l| serde_json::from_str(l).unwrap_or_else(|e| panic!("stdout line {l:?}: {e}")))
        .collect()
}

/// A JSON-RPC `tools/call` request.
pub fn call(id: u32, name: &str, arguments: Value) -> Value {
    serde_json::json!({ "jsonrpc": "2.0", "id": id, "method": "tools/call",
        "params": { "name": name, "arguments": arguments } })
}

impl Drop for Tok {
    fn drop(&mut self) {
        reap(&self.home);
    }
}

/// Keeps a daemon started by `cmd` off the developer's real Claude transcripts: its usage ticker
/// reads `$CLAUDE_CONFIG_DIR/projects` (else `~/.claude/projects`) and, in a fresh data folder,
/// parses all of it from the top, which keeps every test daemon busy for as long as it lives.
/// A folder that does not exist has no transcripts.
pub fn isolate(cmd: &mut Command, home: &Path) {
    cmd.env("CLAUDE_CONFIG_DIR", home.join("no-claude-config"));
}

/// Write ends of the lifeline pipes of the daemons this test binary started. A daemon reads the
/// other end and stops when it closes, which the kernel does when this process dies, even by
/// SIGKILL, so no daemon outlives its test binary.
static LIFELINES: Mutex<Vec<(PathBuf, std::process::ChildStdin)>> = Mutex::new(Vec::new());

fn lifelines() -> std::sync::MutexGuard<'static, Vec<(PathBuf, std::process::ChildStdin)>> {
    LIFELINES.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Is a daemon answering for `home`?
pub fn daemon_healthy(home: &Path) -> bool {
    DaemonInfo::read(&home.join("daemon.json"))
        .ok()
        .flatten()
        .is_some_and(|i| Client::healthy(&i.base_url()))
}

/// Runs `tok daemon start --exit-with-parent --json` in `home`, keeping the pipe it hands to the
/// daemon open for as long as this process lives. Returns what `tok` printed.
pub fn start_daemon(home: &Path) -> Output {
    track(home);
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_tok"));
    cmd.args(["daemon", "start", "--exit-with-parent", "--json"])
        .env("TOKNILA_HOME", home);
    isolate(&mut cmd, home);
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn tok daemon start");
    let stdin = child.stdin.take().expect("stdin");
    lifelines().push((home.to_owned(), stdin));
    child.wait_with_output().expect("wait for tok daemon start")
}

/// Starts `tok watch` (extra `args` after `watch`) that ends when this process dies.
pub fn spawn_watch(home: &Path, args: &[&str]) -> std::process::Child {
    Command::new(env!("CARGO_BIN_EXE_tok"))
        .arg("watch")
        .args(args)
        .arg("--exit-with-parent")
        .env("TOKNILA_HOME", home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn tok watch")
}

/// The data folders whose daemon this test binary may have started.
static HOMES: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());
static HANDLER: Once = Once::new();

fn homes() -> std::sync::MutexGuard<'static, Vec<PathBuf>> {
    HOMES.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Remembers `home`, so an interrupt (Ctrl-C, SIGTERM, SIGHUP) of the test binary stops its
/// daemon too: a detached daemon outlives its test process, and `Drop` does not run on a signal.
pub fn track(home: &Path) {
    HANDLER.call_once(|| {
        let _ = ctrlc::set_handler(|| {
            let all: Vec<PathBuf> = homes().drain(..).collect();
            for home in all {
                reap(&home);
            }
            std::process::exit(130);
        });
    });
    let mut all = homes();
    if !all.iter().any(|h| h == home) {
        all.push(home.to_owned());
    }
}

/// Stops the daemon of `home` and does not return before its process is gone: asks it to stop,
/// waits, and kills it by the pid in this folder's own `daemon.json` if it is still there.
/// Never matches processes by name.
pub fn reap(home: &Path) {
    homes().retain(|h| h != home);
    let pid = DaemonInfo::read(&home.join("daemon.json"))
        .ok()
        .flatten()
        .map(|i| i.pid);
    let _ = Command::new(env!("CARGO_BIN_EXE_tok"))
        .args(["daemon", "stop"])
        .env("TOKNILA_HOME", home)
        .output();
    lifelines().retain(|(h, _)| h != home);
    let Some(pid) = pid else { return };
    let until = Instant::now() + Duration::from_secs(30);
    while pid_alive(pid) && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(50));
    }
    if pid_alive(pid) {
        kill_pid(pid);
        let until = Instant::now() + Duration::from_secs(15);
        while pid_alive(pid) && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

/// Stops the daemon of a data folder when dropped, whether the test passed, failed or was
/// interrupted. For tests that do not use [`Tok`].
pub struct DaemonGuard(PathBuf);

impl DaemonGuard {
    pub fn new(home: &Path) -> Self {
        track(home);
        Self(home.to_owned())
    }
}

impl Drop for DaemonGuard {
    fn drop(&mut self) {
        reap(&self.0);
    }
}

#[cfg(unix)]
pub fn pid_alive(pid: u32) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

#[cfg(unix)]
fn kill_pid(pid: u32) {
    let _ = Command::new("kill").args(["-9", &pid.to_string()]).status();
}

#[cfg(windows)]
pub fn pid_alive(pid: u32) -> bool {
    Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/NH"])
        .output()
        .is_ok_and(|o| String::from_utf8_lossy(&o.stdout).contains(&pid.to_string()))
}

#[cfg(windows)]
fn kill_pid(pid: u32) {
    let _ = Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/F"])
        .status();
}

/// `fake-agent` is built by the daemon's own tests; build it here if this test runs alone.
pub fn fake_agent() -> PathBuf {
    let exe = format!("fake-agent{}", std::env::consts::EXE_SUFFIX);
    let path = Path::new(env!("CARGO_BIN_EXE_tok"))
        .parent()
        .expect("target dir")
        .join(exe);
    if !path.exists() {
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
        let status = Command::new(cargo)
            .args([
                "build",
                "--locked",
                "-p",
                "toknila-daemon",
                "--bin",
                "fake-agent",
            ])
            .status()
            .expect("run cargo");
        assert!(status.success(), "cannot build fake-agent");
    }
    path
}

/// Runs `git -C dir args` with a fixed identity; panics on failure.
pub fn git(dir: &Path, args: &[&str]) {
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

/// `<dir>/repo`: a repository whose only branch is `develop`, with one commit.
pub fn repo(dir: &Path) -> PathBuf {
    let repo = dir.join("repo");
    std::fs::create_dir_all(&repo).expect("mkdir");
    git(&repo, &["init", "--quiet"]);
    git(&repo, &["symbolic-ref", "HEAD", "refs/heads/develop"]);
    std::fs::write(repo.join("README.md"), "hi\n").expect("write");
    git(&repo, &["add", "README.md"]);
    git(&repo, &["commit", "--quiet", "-m", "init"]);
    repo
}

/// Kills a process without letting it clean up (`kill -9` / `TerminateProcess`).
pub fn kill_hard(pid: u32) {
    let pid = pid.to_string();
    #[cfg(unix)]
    let out = Command::new("kill").args(["-9", &pid]).output();
    #[cfg(windows)]
    let out = Command::new("taskkill").args(["/F", "/PID", &pid]).output();
    // The process may already be gone; a missing tool is the only test error.
    out.expect("run kill");
}

/// Replaces what differs between runs (ids, clocks, the temporary folder an imported chain came
/// from) with fixed values, keeping every key.
fn normalize(v: &mut Value) {
    match v {
        Value::Object(m) => {
            for (k, x) in m.iter_mut() {
                let clock = k.ends_with("_at")
                    || k.ends_with("_since")
                    || k.ends_with("_ms")
                    || k == "since"
                    || k == "at";
                if clock && x.is_number() {
                    *x = Value::from(0);
                } else if k == "source" && x.is_string() {
                    *x = Value::from("<path>");
                } else {
                    normalize(x);
                }
            }
        }
        Value::Array(a) => a.iter_mut().for_each(normalize),
        Value::String(s)
            if s.len() == 26
                && s.bytes()
                    .all(|b| b.is_ascii_digit() || b.is_ascii_uppercase()) =>
        {
            *s = "<id>".to_owned();
        }
        _ => {}
    }
}

/// Compares `v`, normalized, with `tests/snapshots/<name>.json`.
pub fn snapshot(name: &str, mut v: Value) {
    normalize(&mut v);
    let text = format!("{}\n", serde_json::to_string_pretty(&v).expect("json"));
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/snapshots")
        .join(format!("{name}.json"));
    if std::env::var_os("TOKNILA_UPDATE_SNAPSHOTS").is_some() {
        std::fs::write(&path, &text).expect("write snapshot");
        return;
    }
    let want = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| {
            panic!(
                "missing snapshot {}; run with TOKNILA_UPDATE_SNAPSHOTS=1",
                path.display()
            )
        })
        .replace("\r\n", "\n");
    assert_eq!(
        text, want,
        "snapshot {name} changed; if intended, rerun with TOKNILA_UPDATE_SNAPSHOTS=1"
    );
}
