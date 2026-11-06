//! Running the tools the machine needs (`git`, `claude`, `gh`): an argument list, no shell, a
//! timeout. The judgement of what they said is `toknila_core::system` and `toknila_core::repo`;
//! this module only runs them and gathers facts, behind a trait so tests inject answers.

use std::io::{ErrorKind, Read};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use toknila_core::repo::{self, GitState, PathKind, RepoCheck, RepoFacts};
use toknila_core::system::{self, Probe, SystemChecks};

/// Time a tool gets before it is stopped.
const TIMEOUT: Duration = Duration::from_secs(10);

/// Runs one program and says what happened.
pub trait Tools: Send + Sync + std::fmt::Debug {
    /// Runs `program` with `args` (no shell) and stops it after `timeout`.
    fn run(&self, program: &str, args: &[&str], timeout: Duration) -> Probe;
}

/// The programs on this machine's `PATH`.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemTools;

fn drain(mut pipe: impl Read + Send + 'static) -> std::thread::JoinHandle<String> {
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = pipe.read_to_end(&mut bytes);
        String::from_utf8_lossy(&bytes).into_owned()
    })
}

impl Tools for SystemTools {
    fn run(&self, program: &str, args: &[&str], timeout: Duration) -> Probe {
        let spawned = Command::new(program)
            .args(args)
            .env("GIT_TERMINAL_PROMPT", "0")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn();
        let mut child = match spawned {
            Ok(c) => c,
            Err(e) if e.kind() == ErrorKind::NotFound => return Probe::Missing,
            Err(e) => return Probe::Failed(e.to_string()),
        };
        let out = child.stdout.take().map(drain);
        let err = child.stderr.take().map(drain);
        let started = Instant::now();
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break Some(status),
                Ok(None) if started.elapsed() >= timeout => {
                    let _ = child.kill();
                    let _ = child.wait();
                    break None;
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(10)),
                Err(e) => return Probe::Failed(e.to_string()),
            }
        };
        let text = |h: Option<std::thread::JoinHandle<String>>| {
            h.and_then(|h| h.join().ok()).unwrap_or_default()
        };
        match status {
            None => Probe::TimedOut,
            Some(s) => Probe::Ran {
                code: s.code(),
                stdout: text(out),
                stderr: text(err),
            },
        }
    }
}

/// Runs git inside `repo`.
fn git(tools: &dyn Tools, repo: &str, args: &[&str]) -> Probe {
    let mut all = vec!["-C", repo];
    all.extend_from_slice(args);
    tools.run("git", &all, TIMEOUT)
}

/// Stats `path` and asks git about it.
fn facts(tools: &dyn Tools, path: &str) -> RepoFacts {
    let kind = match Path::new(path).metadata() {
        Ok(m) if m.is_dir() => PathKind::Dir,
        Ok(_) => PathKind::File,
        Err(_) => PathKind::Missing,
    };
    if kind != PathKind::Dir {
        return RepoFacts {
            path: kind,
            git: GitState::NotARepo,
        };
    }
    let inside = git(tools, path, &["rev-parse", "--is-inside-work-tree"]);
    let git = match inside {
        Probe::Missing => GitState::GitMissing,
        Probe::Ran {
            code: Some(0),
            stdout,
            ..
        } if stdout.trim() == "true" => {
            let heads = git(
                tools,
                path,
                &["for-each-ref", "--format=%(refname:short)", "refs/heads"],
            );
            let branches = match heads {
                Probe::Ran { stdout, .. } => stdout.lines().map(str::to_owned).collect(),
                _ => Vec::new(),
            };
            GitState::Repo { branches }
        }
        _ => GitState::NotARepo,
    };
    RepoFacts { path: kind, git }
}

/// May `path` be a project's repository with `base` as its base branch?
pub fn check_repo(tools: &dyn Tools, path: &str, base: &str) -> RepoCheck {
    let path = path.trim();
    // Nothing to ask git about an empty or relative path; judge reads the text first.
    let facts = if path.is_empty() || !Path::new(path).is_absolute() {
        RepoFacts {
            path: PathKind::Missing,
            git: GitState::NotARepo,
        }
    } else {
        facts(tools, path)
    };
    repo::judge(path, base, &facts)
}

/// Runs the version and login probes of git, claude and gh, in parallel.
pub fn system_checks(tools: &dyn Tools) -> SystemChecks {
    let run = |program: &str, args: &[&str]| tools.run(program, args, TIMEOUT);
    let (git, claude, gh) = std::thread::scope(|s| {
        let git = s.spawn(|| system::git(&run("git", &["--version"])));
        let claude = s.spawn(|| system::claude(&run("claude", &["--version"])));
        let gh = s.spawn(|| {
            let version = run("gh", &["--version"]);
            let auth = matches!(version, Probe::Ran { code: Some(0), .. })
                .then(|| run("gh", &["auth", "status"]));
            system::gh(&version, auth.as_ref())
        });
        let join = |h: std::thread::ScopedJoinHandle<'_, _>| h.join().expect("check thread");
        (join(git), join(claude), join(gh))
    });
    SystemChecks::new(vec![git, claude, gh])
}

#[cfg(test)]
mod tests;
