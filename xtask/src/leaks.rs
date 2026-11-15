//! The leak checks. `cargo xtask verify` ends with one: a `tok daemon run` whose data home is under
//! the run's own root (`TOKNILA_TEST_ROOT`, where the test helpers put their temp homes) is a test
//! daemon that outlived its test. `cargo xtask e2e-web` ends with another: a daemon whose home is a
//! `toknila-e2e-<run id>-` folder of this run, or a `fake-agent` whose command line carries this
//! run's marker `toknila-fa-<run id>-<spec>-<random>` (the specs get the run id in `TOKNILA_E2E_RUN`).
//! Other runs on the machine have other roots and other ids: what they left is only a warning
//! (older than an hour), never killed. Needs `ps`; where there is none (Windows) it checks nothing
//! and says so (the Windows CI job lists the leftovers itself).

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

/// The environment variable that names the folder a run's test homes are created under.
pub const ROOT_VAR: &str = "TOKNILA_TEST_ROOT";
/// The environment variable that carries the id of an `e2e-web` run to its specs.
pub const E2E_RUN_VAR: &str = "TOKNILA_E2E_RUN";
/// What a daemon or a fake agent of another run has to be older than (seconds) to be mentioned.
pub const OLD_AFTER: u64 = 3600;

/// Runs `f` with a fresh run root (`<temp>/toknila-verify-<pid>`), removed afterwards.
pub fn with_run_root(f: impl FnOnce(&Path) -> Result<(), String>) -> Result<(), String> {
    let root = std::env::temp_dir().join(format!("toknila-verify-{}", std::process::id()));
    std::fs::create_dir_all(&root).map_err(|e| format!("{}: {e}", root.display()))?;
    let result = f(&root);
    let _ = std::fs::remove_dir_all(&root);
    result
}

/// The id of this `e2e-web` run: letters and digits only, so a marker splits at its dashes.
pub fn e2e_run_id() -> String {
    format!("r{}", std::process::id())
}

/// One running `tok daemon run`.
#[derive(Debug, PartialEq)]
pub struct Daemon {
    pub pid: u32,
    /// Seconds since it started.
    pub age: u64,
    pub home: PathBuf,
    pub command: String,
}

/// One running `fake-agent`.
#[derive(Debug, PartialEq)]
pub struct Agent {
    pub pid: u32,
    /// Seconds since it started.
    pub age: u64,
    pub command: String,
    /// The `TOKNILA_HOME` of its environment, when it has one.
    pub home: Option<PathBuf>,
}

/// Fails, naming them, when a daemon with a home under `root` is still alive after `grace` (a
/// daemon told to stop takes up to a second). Without a root every temp-home daemon counts.
/// Daemons and fake agents of other runs, older than an hour, are warned about and left alone.
pub fn check(root: Option<&Path>, grace: Duration) -> Result<(), String> {
    let deadline = Instant::now() + grace;
    let mut skipped = true;
    let leaked = loop {
        let Some((all, agents)) = listing() else {
            break Vec::new();
        };
        skipped = false;
        let found = leaked(&all, &scopes(root));
        if found.is_empty() || Instant::now() >= deadline {
            for w in old_leftovers(&all, &agents, &scopes(root), &scopes(None), OLD_AFTER) {
                eprintln!("xtask: warning: {w}");
            }
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

/// Fails, naming pid, command line and spec of each, when the run `run_id` of `e2e-web` left a
/// daemon or a fake agent alive after `grace`. What it names is killed (it is this run's own).
pub fn check_e2e(run_id: &str, grace: Duration) -> Result<(), String> {
    let deadline = Instant::now() + grace;
    let found = loop {
        let Some((all, agents)) = listing() else {
            eprintln!("xtask: e2e leak check skipped (no `ps` here)");
            return Ok(());
        };
        let found = e2e_leftovers(&all, &agents, run_id, &scopes(None));
        if found.is_empty() || Instant::now() >= deadline {
            for w in old_leftovers(&all, &agents, &[], &scopes(None), OLD_AFTER) {
                eprintln!("xtask: warning: {w}");
            }
            break found;
        }
        std::thread::sleep(Duration::from_millis(250));
    };
    if found.is_empty() {
        eprintln!("xtask: no e2e daemon or fake agent left running");
        return Ok(());
    }
    for (pid, _) in &found {
        let _ = Command::new("kill").args(["-9", &pid.to_string()]).status();
    }
    let lines: Vec<String> = found.into_iter().map(|(_, text)| text).collect();
    Err(format!(
        "the e2e run left processes alive (killed now): {}",
        lines.join("; ")
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
    all.iter().filter(|d| under(&d.home, scopes)).collect()
}

fn under(path: &Path, scopes: &[PathBuf]) -> bool {
    scopes
        .iter()
        .any(|t| !t.as_os_str().is_empty() && path.starts_with(t))
}

/// The spec a `toknila-e2e-<run>-<spec>-<random>` or `toknila-fa-<run>-<spec>-<random>` name is of.
fn spec_of<'a>(name: &'a str, prefix: &str) -> &'a str {
    let rest = name.strip_prefix(prefix).unwrap_or(name);
    rest.rsplit_once('-').map_or(rest, |(spec, _)| spec)
}

/// What the run `run_id` left: its test homes' daemons, and the fake agents carrying its marker,
/// as (pid, "pid, command, spec"). `temps` are the folders the specs make their homes in.
pub fn e2e_leftovers(
    daemons: &[Daemon],
    agents: &[Agent],
    run_id: &str,
    temps: &[PathBuf],
) -> Vec<(u32, String)> {
    let home_prefix = format!("toknila-e2e-{run_id}-");
    let marker_prefix = format!("toknila-fa-{run_id}-");
    let mut found = Vec::new();
    for d in daemons {
        let name = d.home.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let in_temp = d.home.parent().is_some_and(|p| under(p, temps));
        if in_temp && name.starts_with(&home_prefix) {
            let spec = spec_of(name, &home_prefix);
            found.push((
                d.pid,
                format!("daemon pid {} `{}` of spec {spec}", d.pid, d.command),
            ));
        }
    }
    for a in agents {
        let marker = a
            .command
            .split_whitespace()
            .find(|w| w.contains(&marker_prefix));
        if let Some(marker) = marker {
            let spec = spec_of(
                &marker[marker.find(&marker_prefix).unwrap_or(0)..],
                &marker_prefix,
            );
            found.push((
                a.pid,
                format!("fake agent pid {} `{}` of spec {spec}", a.pid, a.command),
            ));
        }
    }
    found
}

/// Daemons with a home in a temp folder, and fake agents, that are at least `min_age` seconds old
/// and not under `mine` (this run's root): what another run left. Said in words, never killed.
pub fn old_leftovers(
    daemons: &[Daemon],
    agents: &[Agent],
    mine: &[PathBuf],
    temps: &[PathBuf],
    min_age: u64,
) -> Vec<String> {
    let mut out = Vec::new();
    for d in daemons {
        if d.age >= min_age && under(&d.home, temps) && !under(&d.home, mine) {
            out.push(format!(
                "a daemon from {}, alive for {} h; another run left it (pid {}, not killed)",
                d.home.display(),
                d.age / 3600,
                d.pid
            ));
        }
    }
    for a in agents {
        let theirs = a.home.as_ref().is_none_or(|h| !under(h, mine));
        if a.age >= min_age && theirs {
            let from = a
                .home
                .as_ref()
                .map_or_else(|| format!("`{}`", a.command), |h| h.display().to_string());
            out.push(format!(
                "a fake agent from {from}, alive for {} h; another run left it (pid {}, not killed)",
                a.age / 3600,
                a.pid
            ));
        }
    }
    out
}

/// Every `tok daemon run` and every `fake-agent` of this user, or `None` when `ps` is unavailable.
fn listing() -> Option<(Vec<Daemon>, Vec<Agent>)> {
    let out = Command::new("ps")
        .args(["-axww", "-o", "pid=,etime=,command="])
        .output()
        .ok()
        .filter(|o| o.status.success())?;
    let (mut daemons, mut agents) = (Vec::new(), Vec::new());
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let Some((pid, age, command)) = split_line(line) else {
            continue;
        };
        if is_daemon_run(command) {
            if let Some(home) = home_of(pid) {
                let command = command.to_owned();
                daemons.push(Daemon {
                    pid,
                    age,
                    home,
                    command,
                });
            }
        } else if is_fake_agent(command) {
            let (command, home) = (command.to_owned(), home_of(pid));
            agents.push(Agent {
                pid,
                age,
                command,
                home,
            });
        }
    }
    Some((daemons, agents))
}

/// `pid etime command` of one `ps` line.
fn split_line(line: &str) -> Option<(u32, u64, &str)> {
    let mut parts = line.split_whitespace();
    let pid = parts.next()?.parse().ok()?;
    let age = parse_etime(parts.next()?)?;
    let after_pid = line
        .trim_start()
        .split_once(char::is_whitespace)?
        .1
        .trim_start();
    let command = after_pid.split_once(char::is_whitespace)?.1.trim();
    Some((pid, age, command))
}

/// `pid etime command` of one `ps` line, for a `daemon run` command only.
#[cfg(test)]
pub fn parse_line(line: &str) -> Option<(u32, u64, &str)> {
    split_line(line).filter(|(_, _, command)| is_daemon_run(command))
}

fn is_daemon_run(command: &str) -> bool {
    let words: Vec<&str> = command.split_whitespace().collect();
    let exe = words
        .first()
        .and_then(|w| Path::new(w).file_stem())
        .and_then(|s| s.to_str());
    matches!(exe, Some("tok" | "toknila"))
        && words.get(1) == Some(&"daemon")
        && words.get(2) == Some(&"run")
}

/// The program is `fake-agent`, or a shell or interpreter running a file of that name.
fn is_fake_agent(command: &str) -> bool {
    command
        .split_whitespace()
        .take(2)
        .any(|w| Path::new(w).file_stem().and_then(|s| s.to_str()) == Some("fake-agent"))
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
fn home_of(pid: u32) -> Option<PathBuf> {
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

    fn daemon(pid: u32, age: u64, home: &str) -> Daemon {
        Daemon {
            pid,
            age,
            home: home.into(),
            command: "tok daemon run".into(),
        }
    }

    fn agent(pid: u32, age: u64, command: &str, home: Option<&str>) -> Agent {
        Agent {
            pid,
            age,
            command: command.into(),
            home: home.map(PathBuf::from),
        }
    }

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
        let all = [
            daemon(1, 10, "/tmp/toknila-verify-1/a/home"),
            daemon(2, 9_999, "/tmp/toknila-verify-1/b/home"),
            daemon(3, 10, "/tmp/toknila-verify-2/c/home"),
            daemon(4, 10, "/Users/x/.toknila"),
        ];
        let mine = leaked(&all, &[PathBuf::from("/tmp/toknila-verify-1")]);
        assert_eq!(mine, vec![&all[0], &all[1]]);
        // No root: every temp-home daemon is reported.
        let every = leaked(&all, &[PathBuf::from("/tmp")]);
        assert_eq!(every, vec![&all[0], &all[1], &all[2]]);
        // An empty scope matches nothing.
        assert!(leaked(&all, &[PathBuf::new()]).is_empty());
    }

    #[test]
    fn e2e_leftovers_name_pid_command_and_spec_of_this_run_only() {
        let temps = [PathBuf::from("/tmp")];
        let daemons = [
            daemon(1, 5, "/tmp/toknila-e2e-r7-ask-story-AbC123"),
            daemon(2, 5, "/tmp/toknila-e2e-r8-ask-story-ZzZ999"),
            daemon(3, 5, "/Users/x/.toknila"),
        ];
        let agents = [
            agent(
                4,
                5,
                "/t/fake-agent sleep toknila-fa-r7-lost-answer-Q1w2E3 hold",
                None,
            ),
            agent(
                5,
                5,
                "/t/fake-agent sleep toknila-fa-r8-lost-answer-Q1w2E3 hold",
                None,
            ),
        ];
        let found = e2e_leftovers(&daemons, &agents, "r7", &temps);
        assert_eq!(
            found,
            vec![
                (1, "daemon pid 1 `tok daemon run` of spec ask-story".to_owned()),
                (
                    4,
                    "fake agent pid 4 `/t/fake-agent sleep toknila-fa-r7-lost-answer-Q1w2E3 hold` of spec lost-answer"
                        .to_owned()
                ),
            ]
        );
    }

    #[test]
    fn a_planted_old_daemon_is_a_warning_and_this_runs_own_is_not() {
        let temps = [PathBuf::from("/tmp")];
        let mine = [PathBuf::from("/tmp/toknila-verify-9")];
        let daemons = [
            daemon(1, 26 * 3600, "/tmp/toknila-verify-3/x/home"),
            daemon(2, 26 * 3600, "/tmp/toknila-verify-9/y/home"),
            daemon(3, 600, "/tmp/toknila-verify-3/z/home"),
            daemon(4, 26 * 3600, "/Users/x/.toknila"),
        ];
        let agents = [
            agent(
                5,
                7200,
                "/t/fake-agent hold",
                Some("/tmp/toknila-verify-3/x/home"),
            ),
            agent(
                6,
                7200,
                "/t/fake-agent hold",
                Some("/tmp/toknila-verify-9/y/home"),
            ),
        ];
        let said = old_leftovers(&daemons, &agents, &mine, &temps, OLD_AFTER);
        assert_eq!(
            said,
            vec![
                "a daemon from /tmp/toknila-verify-3/x/home, alive for 26 h; another run left it (pid 1, not killed)",
                "a fake agent from /tmp/toknila-verify-3/x/home, alive for 2 h; another run left it (pid 5, not killed)",
            ]
        );
    }

    /// A shell running a file called `fake-agent` with the run's marker, left alive: the check
    /// fails naming its pid, command and spec; once it is gone the check passes. Unix only: the
    /// check needs `ps` and the script needs `sh`.
    #[cfg(unix)]
    #[test]
    fn a_fake_agent_left_behind_fails_the_e2e_check() {
        use std::process::Stdio;
        let run_id = format!("t{}", std::process::id());
        let marker = format!("toknila-fa-{run_id}-planner-AbC123");
        let dir =
            std::env::temp_dir().join(format!("toknila-xtask-leak-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("the test folder is made");
        let script = dir.join("fake-agent");
        // Blocks on its stdin, which stays open until the test closes it: no sleep, no deadline race.
        std::fs::write(&script, "read _\n").expect("the script is written");
        let mut child = Command::new("sh")
            .arg(&script)
            .args(["print", &marker])
            .stdin(Stdio::piped())
            .spawn()
            .expect("sh starts");
        let err = check_e2e(&run_id, Duration::ZERO).expect_err("the leftover fails the check");
        let _ = child.wait();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(err.contains(&format!("pid {}", child.id())), "{err}");
        assert!(err.contains(&format!("print {marker}")), "{err}");
        assert!(err.contains("of spec planner"), "{err}");
        // The control: with the leftover gone, the same check passes.
        check_e2e(&run_id, Duration::ZERO).expect("nothing is left");
    }
}
