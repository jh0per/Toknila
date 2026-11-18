//! Before the tests, `verify` launches the freshly built `tok` and starts and stops one daemon with
//! it, twice, and says how long each launch took. On a fresh Windows runner the first launches of
//! a new `tok.exe` are slow (the virus scanner reads the new executable): four cli tests failed
//! there with "daemon did not answer within 15 s" near the start of the run (CI run 44136517698,
//! 2026-11-17) while the rest passed. Paid here, once, that cost is a number in the log and not a
//! failing test. After the tests, the times of the tests' own daemon starts are summed up.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use crate::leaks::ROOT_VAR;

/// How long a warm-up daemon may take to answer before `verify` fails: far above any start seen,
/// so only a daemon that hangs reaches it.
const DEADLINE: Duration = Duration::from_secs(120);
/// The file the cli tests' helper writes one line per daemon start to, under the run's root.
pub const STARTS_FILE: &str = "daemon-starts.txt";

/// Launches `tok` twice (`--version`) and starts and stops a daemon twice, printing the times.
pub fn run(tok: &Path, root: &Path) -> Result<(), String> {
    let first = launch(tok)?;
    let second = launch(tok)?;
    eprintln!(
        "xtask verify: warm-up: `tok --version` took {:.2}s, then {:.2}s",
        first.as_secs_f64(),
        second.as_secs_f64()
    );
    for n in 1..=2 {
        let home = root.join(format!("warm-up-{n}"));
        let (file, answer) = daemon(tok, &home, root)?;
        eprintln!(
            "xtask verify: warm-up daemon {n}: address file after {:.2}s, first answer after {:.2}s",
            file.as_secs_f64(),
            answer.as_secs_f64()
        );
    }
    Ok(())
}

fn launch(tok: &Path) -> Result<Duration, String> {
    let begin = Instant::now();
    let out = Command::new(tok)
        .arg("--version")
        .output()
        .map_err(|e| format!("cannot run {}: {e}", tok.display()))?;
    if !out.status.success() {
        return Err(format!(
            "`{} --version` exited with {}",
            tok.display(),
            out.status
        ));
    }
    Ok(begin.elapsed())
}

/// Starts `tok daemon run` in `home`; the times from the spawn to its address file and to its
/// first answer on `/healthz`. The daemon is stopped (its lifeline closed) before this returns.
fn daemon(tok: &Path, home: &Path, root: &Path) -> Result<(Duration, Duration), String> {
    let begin = Instant::now();
    let mut child = Command::new(tok)
        .args(["daemon", "run", "--exit-with-parent"])
        .env("TOKNILA_HOME", home)
        .env(ROOT_VAR, root)
        // No transcripts to read: a test daemon's usage ticker would parse the real ones.
        .env("CLAUDE_CONFIG_DIR", home.join("no-claude-config"))
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("cannot start a daemon with {}: {e}", tok.display()))?;
    let timed = (|| {
        let info = home.join("daemon.json");
        let port = wait(begin, || {
            std::fs::read_to_string(&info)
                .ok()
                .and_then(|t| port_of(&t))
        })
        .ok_or_else(|| format!("no address file {} within {DEADLINE:?}", info.display()))?;
        let file = begin.elapsed();
        wait(begin, || healthy(port).then_some(())).ok_or_else(|| {
            format!("the daemon wrote its address but did not answer within {DEADLINE:?}")
        })?;
        Ok((file, begin.elapsed()))
    })();
    stop(&mut child);
    timed
}

/// Polls `probe` until it gives a value or [`DEADLINE`] after `begin` has passed.
fn wait<T>(begin: Instant, mut probe: impl FnMut() -> Option<T>) -> Option<T> {
    while begin.elapsed() < DEADLINE {
        if let Some(v) = probe() {
            return Some(v);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    None
}

/// The `"port"` of an address file.
fn port_of(json: &str) -> Option<u16> {
    let rest = &json[json.find("\"port\"")? + "\"port\"".len()..];
    let digits: String = rest
        .trim_start_matches([' ', ':'])
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    digits.parse().ok()
}

/// Whether the daemon on `port` answers `GET /healthz` with 200.
fn healthy(port: u16) -> bool {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let Ok(mut s) = TcpStream::connect_timeout(&addr, Duration::from_secs(2)) else {
        return false;
    };
    let _ = s.set_read_timeout(Some(Duration::from_secs(2)));
    let request = "GET /healthz HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n";
    let mut head = [0u8; 12];
    s.write_all(request.as_bytes()).is_ok()
        && s.read_exact(&mut head).is_ok()
        && head.ends_with(b" 200")
}

/// Closes the daemon's lifeline and waits for it to end; kills it after twenty seconds.
fn stop(child: &mut Child) {
    drop(child.stdin.take());
    let until = Instant::now() + Duration::from_secs(20);
    while Instant::now() < until {
        if child.try_wait().ok().flatten().is_some() {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let _ = child.kill();
    let _ = child.wait();
}

/// Prints how long the tests' own daemon starts took (one line per start in [`STARTS_FILE`]:
/// milliseconds since the epoch at the spawn, milliseconds it took, `ok` or `failed`, the test).
pub fn summary(root: &Path) {
    let Ok(text) = std::fs::read_to_string(root.join(STARTS_FILE)) else {
        return;
    };
    let mut starts: Vec<(u128, u128, bool, &str)> = text
        .lines()
        .filter_map(|l| {
            let mut f = l.splitn(4, ' ');
            Some((
                f.next()?.parse().ok()?,
                f.next()?.parse().ok()?,
                f.next()? == "ok",
                f.next().unwrap_or("?"),
            ))
        })
        .collect();
    if starts.is_empty() {
        return;
    }
    starts.sort_by_key(|s| s.0);
    let mut took: Vec<u128> = starts.iter().map(|s| s.1).collect();
    took.sort_unstable();
    let failed = starts.iter().filter(|s| !s.2).count();
    let secs = |ms: u128| ms as f64 / 1000.0;
    eprintln!(
        "xtask verify: test daemon starts: {} ({failed} failed), median {:.2}s, slowest {:.2}s",
        starts.len(),
        secs(took[took.len() / 2]),
        secs(took[took.len() - 1])
    );
    let first: Vec<String> = starts
        .iter()
        .take(8)
        .map(|s| format!("{:.2}s", secs(s.1)))
        .collect();
    eprintln!(
        "xtask verify: the first test daemon starts took {}",
        first.join(" ")
    );
    let mut slow: Vec<&(u128, u128, bool, &str)> = starts.iter().collect();
    slow.sort_by_key(|s| std::cmp::Reverse(s.1));
    for s in slow.iter().take(3) {
        eprintln!(
            "xtask verify: slow test daemon start: {:.2}s {} ({})",
            secs(s.1),
            s.3,
            if s.2 { "ok" } else { "failed" }
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_port_is_read_from_an_address_file() {
        let json = r#"{"pid": 7, "port": 51234, "token": "t", "version": "0.3.8"}"#;
        assert_eq!(port_of(json), Some(51234));
        assert_eq!(port_of(r#"{"port":80}"#), Some(80));
        assert_eq!(port_of(r#"{"pid": 7}"#), None);
    }
}
