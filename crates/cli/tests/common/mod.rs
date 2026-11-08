//! Helpers shared by the end-to-end tests: the real `tok` binary against a temporary data
//! folder, and JSON snapshots in `tests/snapshots/` (`TOKNILA_UPDATE_SNAPSHOTS=1` rewrites them
//! after an intended change).

// Each test file uses only some helpers.
#![allow(dead_code)]

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

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
        Self {
            home: dir.join("home"),
        }
    }

    /// `tok <args>` against this data folder, not yet run.
    fn command(&self, args: &[&str]) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_tok"));
        c.args(args).env("TOKNILA_HOME", &self.home);
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
        let _ = self.command(&["daemon", "stop"]).output();
    }
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
