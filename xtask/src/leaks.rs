//! The leak check that ends `cargo xtask verify`: a `tok daemon run` whose data home is under the
//! run's own root (`TOKNILA_TEST_ROOT`, where the test helpers put their temp homes) is a test
//! daemon that outlived its test. Other runs on the machine have other roots.
//! Needs `ps`; where there is none (Windows) it checks nothing and says so.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

/// The environment variable that names the folder a run's test homes are created under.
pub const ROOT_VAR: &str = "TOKNILA_TEST_ROOT";

/// Runs `f` with a fresh run root (`<temp>/toknila-verify-<pid>`), removed afterwards.
pub fn with_run_root(f: impl FnOnce(&Path) -> Result<(), String>) -> Result<(), String> {
    let root = std::env::temp_dir().join(format!("toknila-verify-{}", std::process::id()));
    std::fs::create_dir_all(&root).map_err(|e| format!("{}: {e}", root.display()))?;
    let result = f(&root);
    let _ = std::fs::remove_dir_all(&root);
    result
}

/// One running `tok daemon run`.
#[derive(Debug, PartialEq)]
pub struct Daemon {
    pub pid: u32,
    /// Seconds since it started.
    pub age: u64,
    pub home: PathBuf,
}

/// Fails, naming them, when a daemon with a home under `root` is still alive after `grace` (a
/// daemon told to stop takes up to a second). Without a root every temp-home daemon counts.
pub fn check(root: Option<&Path>, grace: Duration) -> Result<(), String> {
    let deadline = Instant::now() + grace;
    let mut skipped = true;
    let leaked = loop {
        let Some(all) = daemons() else {
            break Vec::new();
        };
        skipped = false;
        let found = leaked(&all, &scopes(root));
        if found.is_empty() || Instant::now() >= deadline {
            break found.into_iter().map(describe).collect();
        }
        std::thread::sleep(Duration::from_millis(250));
    };
    if skipped {
        eprintln!("xtask: daemon leak check skipped (no `ps` here)");
        return Ok(());
    }
    if leaked.is_empty() {
        eprintln!("xtask: no test daemon left running");
        return Ok(());
    }
    Err(format!(
        "test daemons of this run are still alive: {}",
        leaked.join("; ")
    ))
}

fn describe(d: &Daemon) -> String {
    format!("pid {} (home {}, up {}s)", d.pid, d.home.display(), d.age)
}

/// The folders a leaked daemon's home may be under: the root (as given and resolved), or the
/// temp dir when there is no root.
fn scopes(root: Option<&Path>) -> Vec<PathBuf> {
    let base = root.map_or_else(std::env::temp_dir, Path::to_path_buf);
    vec![base.canonicalize().unwrap_or_default(), base]
}

/// The daemons whose home is under one of `scopes`.
pub fn leaked<'a>(all: &'a [Daemon], scopes: &[PathBuf]) -> Vec<&'a Daemon> {
    all.iter()
        .filter(|d| {
            scopes
                .iter()
                .any(|t| !t.as_os_str().is_empty() && d.home.starts_with(t))
        })
        .collect()
}

/// Every `tok daemon run` of this user, or `None` when `ps` is unavailable.
fn daemons() -> Option<Vec<Daemon>> {
    let out = Command::new("ps")
        .args(["-axww", "-o", "pid=,etime=,command="])
        .output()
        .ok()
        .filter(|o| o.status.success())?;
    let mut found = Vec::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let Some((pid, age, command)) = parse_line(line) else {
            continue;
        };
        if let Some(home) = home_of(pid, command) {
            found.push(Daemon { pid, age, home });
        }
    }
    Some(found)
}

/// `pid etime command` of one `ps` line, for a `daemon run` command only.
pub fn parse_line(line: &str) -> Option<(u32, u64, &str)> {
    let mut parts = line.split_whitespace();
    let pid = parts.next()?.parse().ok()?;
    let age = parse_etime(parts.next()?)?;
    let after_pid = line
        .trim_start()
        .split_once(char::is_whitespace)?
        .1
        .trim_start();
    let command = after_pid.split_once(char::is_whitespace)?.1.trim_start();
    let words: Vec<&str> = command.split_whitespace().collect();
    let exe = Path::new(words.first()?).file_stem()?.to_str()?;
    let runs_daemon = words.get(1) == Some(&"daemon") && words.get(2) == Some(&"run");
    (matches!(exe, "tok" | "toknila") && runs_daemon).then_some((pid, age, command.trim()))
}

/// `[[dd-]hh:]mm:ss` as seconds.
pub fn parse_etime(s: &str) -> Option<u64> {
    let (days, rest) = match s.split_once('-') {
        Some((d, r)) => (d.parse::<u64>().ok()?, r),
        None => (0, s),
    };
    let mut secs = 0;
    for part in rest.split(':') {
        secs = secs * 60 + part.parse::<u64>().ok()?;
    }
    Some(days * 86_400 + secs)
}

/// The `TOKNILA_HOME` in the environment of `pid`.
fn home_of(pid: u32, _command: &str) -> Option<PathBuf> {
    let out = Command::new("ps")
        .args(["eww", "-o", "command=", "-p", &pid.to_string()])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let rest = text.split("TOKNILA_HOME=").nth(1)?;
    Some(PathBuf::from(rest.split_whitespace().next()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn etime_forms() {
        assert_eq!(parse_etime("00:07"), Some(7));
        assert_eq!(parse_etime("02:03:04"), Some(7384));
        assert_eq!(parse_etime("4-00:00:01"), Some(345_601));
        assert_eq!(parse_etime("x"), None);
    }

    #[test]
    fn only_daemon_run_of_tok_is_listed() {
        let ok = "  123  01:02 /x/target/debug/tok daemon run --exit-with-parent";
        assert_eq!(parse_line(ok).map(|p| (p.0, p.1)), Some((123, 62)));
        assert!(parse_line(" 5 00:01 /x/tok daemon status").is_none());
        assert!(parse_line(" 5 00:01 vim daemon run").is_none());
    }

    #[test]
    fn leaks_are_homes_under_the_scope_whatever_their_age() {
        let d = |pid, age, home: &str| Daemon {
            pid,
            age,
            home: home.into(),
        };
        let all = [
            d(1, 10, "/tmp/toknila-verify-1/a/home"),
            d(2, 9_999, "/tmp/toknila-verify-1/b/home"),
            d(3, 10, "/tmp/toknila-verify-2/c/home"),
            d(4, 10, "/Users/x/.toknila"),
        ];
        let mine = leaked(&all, &[PathBuf::from("/tmp/toknila-verify-1")]);
        assert_eq!(mine, vec![&all[0], &all[1]]);
        // No root: every temp-home daemon is reported.
        let every = leaked(&all, &[PathBuf::from("/tmp")]);
        assert_eq!(every, vec![&all[0], &all[1], &all[2]]);
        // An empty scope matches nothing.
        assert!(leaked(&all, &[PathBuf::new()]).is_empty());
    }
}
