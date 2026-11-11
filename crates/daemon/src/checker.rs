//! Running the project's check command on a ship: through the platform's shell, in the ship's
//! directory, with a time limit. Standard output and error are kept together, in order, and only
//! the end of them (where a failure is) is held in memory.

use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
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
    /// Set when the daemon stops: every running command is ended, and none starts.
    stopped: Arc<AtomicBool>,
}

impl Default for ShellChecker {
    fn default() -> Self {
        Self::new(CHECK_TIMEOUT)
    }
}

impl ShellChecker {
    /// A checker that stops a command after `limit`.
    pub fn new(limit: Duration) -> Self {
        Self {
            limit,
            stopped: Arc::new(AtomicBool::new(false)),
        }
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
    fn stop_all(&self) {
        self.stopped.store(true, Ordering::SeqCst);
    }

    fn run(&self, spec: &CheckSpec) -> Result<CheckResult, CheckError> {
        let started = Instant::now();
        let result = self.run_inner(spec);
        let status = match &result {
            Ok(r) if r.timed_out => "timed out".to_owned(),
            Ok(r) => format!("exit code {:?}", r.code),
            Err(e) => format!("error: {e}"),
        };
        tracing::info!(
            dir = %spec.dir,
            status = %status,
            seconds = started.elapsed().as_secs_f32(),
            "check command ended"
        );
        result
    }
}

impl ShellChecker {
    fn run_inner(&self, spec: &CheckSpec) -> Result<CheckResult, CheckError> {
        if self.stopped.load(Ordering::SeqCst) {
            return Err(CheckError::Io("the daemon is stopping".to_owned()));
        }
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
            if self.stopped.load(Ordering::SeqCst) {
                crate::git::kill_tree(&mut child);
                let _ = child.wait();
                return Err(CheckError::Io("the daemon is stopping".to_owned()));
            }
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

    const PROBE: &str = "TOKNILA_PATH_PROBE";

    /// Runs only inside the child of the test below: a daemon-like process that adopts the
    /// agents' `PATH` and then runs a check whose program lives in `$HOME/.cargo/bin`.
    #[test]
    fn path_probe_child() {
        if std::env::var_os(PROBE).is_none() {
            return;
        }
        crate::adopt_agent_path();
        let dir = tempfile::tempdir().expect("dir");
        let r = ShellChecker::default()
            .run(&spec("fake-tool-for-the-probe", dir.path()))
            .expect("runs");
        assert!(r.success, "{}", r.output);
        assert_eq!(r.output.trim(), "found");
    }

    #[test]
    fn a_check_finds_a_program_in_a_tool_folder_the_daemons_own_path_lacks() {
        use std::os::unix::fs::PermissionsExt;
        let home = tempfile::tempdir().expect("home");
        let bin = home.path().join(".cargo").join("bin");
        std::fs::create_dir_all(&bin).expect("bin");
        let tool = bin.join("fake-tool-for-the-probe");
        std::fs::write(&tool, "#!/bin/sh\necho found\n").expect("tool");
        std::fs::set_permissions(&tool, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        let out = Command::new(std::env::current_exe().expect("exe"))
            .args(["--exact", "checker::tests::path_probe_child", "--nocapture"])
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", home.path())
            .env(PROBE, "1")
            .output()
            .expect("child");
        assert!(
            out.status.success(),
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        // Control: the same child without the tool folder on disk cannot find the program.
        std::fs::remove_file(&tool).expect("remove");
        let out = Command::new(std::env::current_exe().expect("exe"))
            .args(["--exact", "checker::tests::path_probe_child"])
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", home.path())
            .env(PROBE, "1")
            .output()
            .expect("child");
        assert!(
            !out.status.success(),
            "the probe must fail without the tool"
        );
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
    fn stopping_the_checker_kills_a_running_command_and_it_is_an_error_not_a_result() {
        let dir = tempfile::tempdir().expect("dir");
        let checker = ShellChecker::default();
        let stopper = checker.clone();
        let marker = dir.path().join("started");
        let handle = std::thread::spawn({
            let spec = spec("touch started; echo started; sleep 30", dir.path());
            move || checker.run(&spec)
        });
        // The command is running once it has made its mark; a ceiling, not a delay.
        let until = Instant::now() + Duration::from_secs(60);
        while !marker.exists() {
            assert!(Instant::now() < until, "the command never started");
            std::thread::sleep(Duration::from_millis(10));
        }
        let started = Instant::now();
        stopper.stop_all();
        let r = handle.join().expect("joins");
        assert!(matches!(r, Err(CheckError::Io(_))), "{r:?}");
        assert!(started.elapsed() < Duration::from_secs(5), "it was killed");
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
