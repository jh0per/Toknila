//! A stand-in for a CLI coding agent, for tests of the PTY runner on every platform: no shell and
//! no OS tools needed. Its arguments are steps, run in order:
//!
//! - `print <text>`: print a line.
//! - `pwd`: print the working directory.
//! - `env <NAME>`: print `NAME=<value>`.
//! - `echo`: read one line from the terminal and print `got:<line>`.
//! - `sleep <ms>`: wait.
//! - `hold`: wait until killed.
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

fn main() {
    let mut args = std::env::args().skip(1);
    while let Some(step) = args.next() {
        match step.as_str() {
            "print" => say(&args.next().unwrap_or_default()),
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
            "sleep" => {
                let ms = args.next().and_then(|s| s.parse().ok()).unwrap_or(0);
                std::thread::sleep(Duration::from_millis(ms));
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
