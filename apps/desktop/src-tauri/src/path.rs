//! The `PATH` the daemon starts with. An app opened from Finder gets the system's minimal
//! `PATH`, which has neither `claude` nor `gh`; the person's shell has. So the shell is asked for
//! its `PATH` once, and the usual install folders are added behind it.

use std::time::Duration;

const MARK: &str = "__TOKNILA_PATH__";

/// Folders `gh` and the system tools are commonly in.
const USUAL: &[&str] = &[
    "/opt/homebrew/bin",
    "/usr/local/bin",
    "/usr/bin",
    "/bin",
    "/usr/sbin",
    "/sbin",
];

/// The `PATH` for the daemon: the login shell's, else the current one, plus the usual folders.
/// Never empty.
pub fn for_daemon() -> String {
    let current = std::env::var("PATH").unwrap_or_default();
    let from_shell = login_shell_path(Duration::from_secs(5));
    merge(&[from_shell.as_deref().unwrap_or(""), &current])
}

/// Joins the `PATH` strings in order, drops repeats and empty parts, and appends the usual
/// folders.
pub fn merge(paths: &[&str]) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut add = |p: &str| {
        if !p.is_empty() && !out.iter().any(|x| x == p) {
            out.push(p.to_owned());
        }
    };
    paths
        .iter()
        .flat_map(|s| s.split(':'))
        .for_each(&mut add);
    USUAL.iter().for_each(|p| add(p));
    out.join(":")
}

/// What an interactive login shell prints between the markers, whatever its start files also say.
pub fn parse_marked(output: &str) -> Option<String> {
    let rest = output.split_once(MARK)?.1;
    let path = rest.split_once(MARK)?.0.trim();
    (!path.is_empty()).then(|| path.to_owned())
}

#[cfg(unix)]
fn login_shell_path(limit: Duration) -> Option<String> {
    use std::process::{Command, Stdio};
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_owned());
    let child = Command::new(shell)
        .args(["-ilc", &format!("printf '{MARK}%s{MARK}' \"$PATH\"")])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let pid = child.id();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(child.wait_with_output());
    });
    match rx.recv_timeout(limit) {
        Ok(Ok(out)) => parse_marked(&String::from_utf8_lossy(&out.stdout)),
        _ => {
            // A shell that waits for input or a slow start file: give up on it.
            let _ = Command::new("kill").args(["-9", &pid.to_string()]).status();
            None
        }
    }
}

#[cfg(not(unix))]
fn login_shell_path(_: Duration) -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_marked_path_survives_noise_from_start_files() {
        let out = format!("Last login: today\nhello\n{MARK}/a/bin:/b/bin{MARK}\nbye");
        assert_eq!(parse_marked(&out).as_deref(), Some("/a/bin:/b/bin"));
        assert_eq!(parse_marked("no markers"), None);
        assert_eq!(parse_marked(&format!("{MARK}{MARK}")), None);
    }

    #[test]
    fn merge_keeps_order_drops_repeats_and_adds_the_usual_folders() {
        let merged = merge(&["/x:/usr/bin", "/x:/y"]);
        assert_eq!(
            merged,
            "/x:/usr/bin:/y:/opt/homebrew/bin:/usr/local/bin:/bin:/usr/sbin:/sbin"
        );
    }
}
