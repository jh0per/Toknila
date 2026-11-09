//! Running the project's check command on a ship: through the platform's shell, in the ship's
//! directory, with a time limit. Standard output and error are kept together, in order, and only
//! the end of them (where a failure is) is held in memory.

use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use toknila_app::{CheckError, CheckResult, CheckSpec, Checker};

/// How long a check may run before it is stopped and counts as failed.
pub const CHECK_TIMEOUT: Duration = Duration::from_secs(30 * 60);

/// The most bytes of output kept (the end of it).
const KEEP: usize = 256 * 1024;

/// Runs check commands through the shell.
#[derive(Debug, Clone)]
pub struct ShellChecker {
    limit: Duration,
}

impl Default for ShellChecker {
    fn default() -> Self {
        Self::new(CHECK_TIMEOUT)
    }
}

impl ShellChecker {
    /// A checker that stops a command after `limit`.
    pub fn new(limit: Duration) -> Self {
        Self { limit }
    }
}

/// The shell invocation of `command` with the error stream joined to the output.
fn shell(command: &str) -> Command {
    #[cfg(windows)]
    {
        let mut c = Command::new("cmd");
        c.arg("/C").arg(format!("{command} 2>&1"));
        c
    }
    #[cfg(not(windows))]
    {
        let mut c = Command::new("sh");
        c.arg("-c").arg(format!("exec 2>&1; {command}"));
        c
    }
}

/// Reads `pipe` on a thread and keeps its last [`KEEP`] bytes.
fn drain(pipe: Option<impl Read + Send + 'static>) -> std::sync::mpsc::Receiver<Vec<u8>> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut kept = Vec::new();
        let mut chunk = [0_u8; 8192];
        if let Some(mut p) = pipe {
            while let Ok(n) = p.read(&mut chunk) {
                if n == 0 {
                    break;
                }
                kept.extend_from_slice(&chunk[..n]);
                if kept.len() > 2 * KEEP {
                    kept.drain(..kept.len() - KEEP);
                }
            }
        }
        let from = kept.len().saturating_sub(KEEP);
        let _ = tx.send(kept.split_off(from));
    });
    rx
}

impl Checker for ShellChecker {
    fn run(&self, spec: &CheckSpec) -> Result<CheckResult, CheckError> {
        let mut cmd = shell(&spec.command);
        cmd.current_dir(&spec.dir)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // Its own process group, so the time limit can end the command and what it started.
        #[cfg(unix)]
        std::os::unix::process::CommandExt::process_group(&mut cmd, 0);
        let mut child = cmd
            .spawn()
            .map_err(|e| CheckError::Io(format!("cannot run the shell in {}: {e}", spec.dir)))?;
        let out = drain(child.stdout.take());
        let err = drain(child.stderr.take());
        let started = Instant::now();
        let (status, timed_out) = loop {
            match child.try_wait() {
                Ok(Some(status)) => break (Some(status), false),
                Ok(None) if started.elapsed() >= self.limit => {
                    crate::git::kill_tree(&mut child);
                    let _ = child.wait();
                    break (None, true);
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(20)),
                Err(e) => return Err(CheckError::Io(format!("cannot wait for the check: {e}"))),
            }
        };
        // Not waited for long after a kill: a helper of the command may still hold the pipe.
        let grace = Duration::from_secs(2);
        let mut bytes = out.recv_timeout(grace).unwrap_or_default();
        bytes.extend(err.recv_timeout(grace).unwrap_or_default());
        Ok(CheckResult {
            success: status.is_some_and(|s| s.success()),
            code: status.and_then(|s| s.code()),
            timed_out,
            output: String::from_utf8_lossy(&bytes).into_owned(),
        })
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn spec(command: &str, dir: &std::path::Path) -> CheckSpec {
        CheckSpec {
            command: command.to_owned(),
            dir: dir.to_string_lossy().into_owned(),
        }
    }

    #[test]
    fn a_passing_command_has_its_output_and_runs_in_the_directory() {
        let dir = tempfile::tempdir().expect("dir");
        let r = ShellChecker::default()
            .run(&spec("pwd; echo to-stderr >&2", dir.path()))
            .expect("runs");
        assert!(r.success && r.code == Some(0) && !r.timed_out);
        let name = dir.path().file_name().expect("name").to_string_lossy();
        assert!(r.output.contains(name.as_ref()), "{}", r.output);
        assert!(r.output.contains("to-stderr"), "both streams: {}", r.output);
    }

    #[test]
    fn a_failing_command_has_its_exit_code_and_output() {
        let dir = tempfile::tempdir().expect("dir");
        let r = ShellChecker::default()
            .run(&spec("echo boom; exit 7", dir.path()))
            .expect("runs");
        assert_eq!((r.success, r.code, r.timed_out), (false, Some(7), false));
        assert_eq!(r.output.trim(), "boom");
    }

    #[test]
    fn a_command_over_the_time_limit_is_stopped_and_fails() {
        let dir = tempfile::tempdir().expect("dir");
        let started = Instant::now();
        let r = ShellChecker::new(Duration::from_millis(200))
            .run(&spec("echo started; sleep 30", dir.path()))
            .expect("runs");
        assert!(r.timed_out && !r.success, "{r:?}");
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "it was stopped"
        );
        assert!(r.output.contains("started"), "{}", r.output);
    }

    #[test]
    fn only_the_end_of_a_long_output_is_kept() {
        let dir = tempfile::tempdir().expect("dir");
        let r = ShellChecker::default()
            .run(&spec(
                "i=0; while [ $i -lt 40000 ]; do echo line-$i-padding-padding; i=$((i+1)); done",
                dir.path(),
            ))
            .expect("runs");
        assert!(r.output.len() <= KEEP * 2);
        assert!(r.output.trim_end().ends_with("line-39999-padding-padding"));
    }

    #[test]
    fn a_missing_directory_is_an_error_not_a_result() {
        let r = ShellChecker::default().run(&CheckSpec {
            command: "true".to_owned(),
            dir: "/definitely/not/here".to_owned(),
        });
        assert!(matches!(r, Err(CheckError::Io(_))), "{r:?}");
    }
}
