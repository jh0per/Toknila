//! The leak check that ends `cargo xtask verify`: a `tok daemon run` whose data home is under the
//! temporary folder and that started during the run is a test daemon that outlived its test.
//! Needs `ps`; where there is none (Windows) it checks nothing and says so.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

/// One running `tok daemon run`.
#[derive(Debug, PartialEq)]
pub struct Daemon {
    pub pid: u32,
    /// Seconds since it started.
    pub age: u64,
    pub home: PathBuf,
}

/// Fails, naming them, when a daemon of this run (started within `run_age`) is still alive.
pub fn check(run_age: Duration) -> Result<(), String> {
    let Some(all) = daemons() else {
        eprintln!("xtask: daemon leak check skipped (no `ps` here)");
        return Ok(());
    };
    let temp = [
        std::env::temp_dir(),
        std::env::temp_dir().canonicalize().unwrap_or_default(),
    ];
    let leaked = leaked(&all, &temp, run_age.as_secs() + 2);
    if leaked.is_empty() {
        eprintln!("xtask: no test daemon left running");
        return Ok(());
    }
    let list: Vec<String> = leaked
        .iter()
        .map(|d| format!("pid {} (home {}, up {}s)", d.pid, d.home.display(), d.age))
        .collect();
    Err(format!(
        "test daemons of this run are still alive: {}",
        list.join("; ")
    ))
}

/// The daemons under a temporary folder that are no older than `max_age` seconds.
pub fn leaked<'a>(all: &'a [Daemon], temp: &[PathBuf], max_age: u64) -> Vec<&'a Daemon> {
    all.iter()
        .filter(|d| d.age <= max_age)
        .filter(|d| {
            temp.iter()
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
    fn leaks_are_temp_homes_started_during_the_run() {
        let d = |pid, age, home: &str| Daemon {
            pid,
            age,
            home: home.into(),
        };
        let all = [
            d(1, 10, "/tmp/a/home"),
            d(2, 9_999, "/tmp/b/home"),
            d(3, 10, "/Users/x/.toknila"),
        ];
        let got = leaked(&all, &[PathBuf::from("/tmp")], 100);
        assert_eq!(got, vec![&all[0]]);
    }
}
