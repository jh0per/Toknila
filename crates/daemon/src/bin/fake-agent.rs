//! A stand-in for a CLI coding agent, for tests of the PTY runner on every platform: no shell and
//! no OS tools needed. Its arguments are steps, run in order:
//!
//! - `print <text>`: print a line.
//! - `osc52 <base64>`: ask the terminal to put the decoded text on the clipboard (`ESC]52;c;<base64>`).
//! - `pwd`: print the working directory.
//! - `env <NAME>`: print `NAME=<value>`.
//! - `echo`: read one line from the terminal and print `got:<line>`.
//! - `size`: print `size=<cols>x<rows>`, the terminal's size at that moment.
//! - `post <path> <json>`: POST the JSON to the daemon with the run's agent token, as a hook
//!   would, and print `post=<status> <body>`. `{ticket}` in the path is the run's ticket key. The
//!   daemon's address, the token and the ticket come from the `TOKNILA_*` environment the runner
//!   sets. This is how a test plays the hooks of a real agent (`/activity`, `/permission`).
//! - `patch <path> <json>`: the same with PATCH (the planner's `edit_card`).
//! - `stop`: post the `hook:stop` activity, as an agent's Stop hook does when a turn ends (a
//!   reviewer that ends a turn without a verdict).
//! - `trust`: show a folder trust dialog whose options are not numbered, with "No" preselected, and
//!   read one line of keys: a Down arrow before Enter prints `trust=yes`, anything else
//!   `trust=no`.
//! - `pid`: print `pid=<own process id>`, so a test can end the agent from outside.
//! - `flagfile <flag>`: read the file named by the argument after `<flag>` in this process's
//!   whole command line, last occurrence (the instructions file a runner appends) and print each line as `file:<line>`.
//! - `readcheck <flag> <path>`: judge a `Read` of `path` (`{cwd}` is the working directory) by
//!   the settings file named after `<flag>` in the command line, and print
//!   `read allow=<yes|no> deny=<yes|no> <path>`.
//! - `sleep <ms>`: wait.
//! - `hold`: wait until killed.
//! - `child`: start a child process that holds until killed and print `child=<pid>`.
//! - `exit <code>`: exit with that code.
//!
//! Without `exit` it exits with 0 after the last step. Built only to be run by the daemon's tests.

use std::io::{BufRead, Write};
use std::time::Duration;

fn say(line: &str) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{line}");
    let _ = out.flush();
}

/// A command that holds until killed. On Unix it ignores the hangup that closing the terminal
/// sends (as a dev server started with `nohup` does), so only a signal to the group ends it.
#[cfg(unix)]
fn child_command() -> std::process::Command {
    let mut cmd = std::process::Command::new("sh");
    cmd.args(["-c", "trap '' HUP; exec sleep 3600"]);
    cmd
}

#[cfg(not(unix))]
fn child_command() -> std::process::Command {
    let exe = std::env::current_exe().expect("the program's own path");
    let mut cmd = std::process::Command::new(exe);
    cmd.arg("hold");
    cmd
}

/// One HTTP/1.1 POST with a JSON body to the daemon; the status line's code and the body.
fn post(path: &str, json: &str) -> String {
    send("POST", path, json)
}

/// [`post`] with another method (`PATCH`).
fn send(method: &str, path: &str, json: &str) -> String {
    use std::io::Read;
    let var = |name: &str| std::env::var(name).unwrap_or_default();
    let url = var("TOKNILA_URL");
    let host = url
        .trim_start_matches("http://")
        .trim_end_matches('/')
        .to_owned();
    let path = path.replace("{ticket}", &var("TOKNILA_TICKET"));
    let request = format!(
        "{method} /v1{path} HTTP/1.1\r\nHost: {host}\r\nAuthorization: Bearer {}\r\n\
         Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{json}",
        var("TOKNILA_RUN_TOKEN"),
        json.len()
    );
    let Ok(mut stream) = std::net::TcpStream::connect(&host) else {
        return "post=unreachable".to_owned();
    };
    let mut reply = String::new();
    let sent = stream.write_all(request.as_bytes()).is_ok();
    if !sent || stream.read_to_string(&mut reply).is_err() {
        return "post=failed".to_owned();
    }
    let (head, body) = reply.split_once("\r\n\r\n").unwrap_or((&reply, ""));
    let status = head.split_whitespace().nth(1).unwrap_or("?");
    format!("post={status} {}", body.trim())
}

/// Whether one permission rule of the settings file lets a `Read` of `path` through: a bare tool
/// name allows any path, `Read(//dir/**)` a folder, `Read(//file)` one file.
fn read_rule_matches(rule: &str, path: &str) -> bool {
    if matches!(rule, "Read" | "Grep" | "Glob") {
        return true;
    }
    let Some(body) = rule
        .strip_prefix("Read(/")
        .and_then(|r| r.strip_suffix(')'))
    else {
        return false;
    };
    match body.strip_suffix("/**") {
        Some(dir) => path.starts_with(&format!("{dir}/")),
        None => body == path,
    }
}

/// `read allow=<yes|no> deny=<yes|no> <path>`: what the settings file named by `flag` in this
/// process's command line says about reading `path`, the allow and the deny rules apart. `{cwd}` in
/// the path is the working directory.
fn readcheck(flag: &str, target: &str) -> String {
    let cwd = std::env::current_dir()
        .map(|d| d.display().to_string())
        .unwrap_or_default();
    let path = target.replace("{cwd}", &cwd);
    let all: Vec<String> = std::env::args().collect();
    let file = all.windows(2).rfind(|w| w[0] == flag).map(|w| w[1].clone());
    let json: serde_json::Value = file
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    let hit = |list: &str| {
        json["permissions"][list]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|r| r.as_str())
            .any(|r| read_rule_matches(r, &path))
    };
    let yes = |b: bool| if b { "yes" } else { "no" };
    format!(
        "read allow={} deny={} {path}",
        yes(hit("allow")),
        yes(hit("deny"))
    )
}

fn main() {
    let mut args = std::env::args().skip(1);
    while let Some(step) = args.next() {
        match step.as_str() {
            "print" => say(&args.next().unwrap_or_default()),
            // Asks the terminal to copy: the argument is the base64 payload, as an agent sends it.
            "osc52" => say(&format!(
                "\x1b]52;c;{}\x07",
                args.next().unwrap_or_default()
            )),
            "pwd" => say(&std::env::current_dir()
                .map(|d| d.display().to_string())
                .unwrap_or_default()),
            "env" => {
                let name = args.next().unwrap_or_default();
                say(&format!(
                    "{name}={}",
                    std::env::var(&name).unwrap_or_default()
                ));
            }
            "echo" => {
                let mut line = String::new();
                let _ = std::io::stdin().lock().read_line(&mut line);
                say(&format!("got:{}", line.trim()));
            }
            "size" => {
                let size = terminal_size::terminal_size();
                say(&match size {
                    Some((terminal_size::Width(cols), terminal_size::Height(rows))) => {
                        format!("size={cols}x{rows}")
                    }
                    None => "size=unknown".to_owned(),
                });
            }
            "post" => {
                let path = args.next().unwrap_or_default();
                let json = args.next().unwrap_or_default();
                say(&post(&path, &json));
            }
            "patch" => {
                let path = args.next().unwrap_or_default();
                let json = args.next().unwrap_or_default();
                say(&send("PATCH", &path, &json));
            }
            "stop" => say(&post(
                "/tickets/{ticket}/activity",
                r#"{"signal":"hook:stop"}"#,
            )),
            "pid" => say(&format!("pid={}", std::process::id())),
            "flagfile" => {
                let flag = args.next().unwrap_or_default();
                let all: Vec<String> = std::env::args().collect();
                let path = all.windows(2).rfind(|w| w[0] == flag).map(|w| w[1].clone());
                let text = path.and_then(|p| std::fs::read_to_string(p).ok());
                for line in text.unwrap_or_default().lines() {
                    say(&format!("file:{line}"));
                }
            }
            "readcheck" => {
                let flag = args.next().unwrap_or_default();
                let target = args.next().unwrap_or_default();
                say(&readcheck(&flag, &target));
            }
            "trust" => {
                for line in [
                    "Do you trust the files in this folder?",
                    "> No, exit",
                    "  Yes, I trust this folder",
                ] {
                    say(line);
                }
                let mut keys = String::new();
                let _ = std::io::stdin().lock().read_line(&mut keys);
                say(if keys.contains("\x1b[B") {
                    "trust=yes"
                } else {
                    "trust=no"
                });
            }
            "sleep" => {
                let ms = args.next().and_then(|s| s.parse().ok()).unwrap_or(0);
                std::thread::sleep(Duration::from_millis(ms));
            }
            "child" => {
                // Left running on purpose: the test is that stopping the agent ends it.
                #[allow(clippy::zombie_processes)]
                let child = child_command().spawn().expect("the child starts");
                say(&format!("child={}", child.id()));
            }
            "hold" => loop {
                std::thread::sleep(Duration::from_secs(3600));
            },
            "exit" => {
                let code = args.next().and_then(|s| s.parse().ok()).unwrap_or(0);
                std::process::exit(code);
            }
            other => {
                say(&format!("unknown step {other}"));
                std::process::exit(64);
            }
        }
    }
}
