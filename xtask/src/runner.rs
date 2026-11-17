//! Running the release tools (`codesign`, `hdiutil`, `xcrun`, `spctl`, `tar`, `npx`, `gh`) so that
//! the value of a signing setting never reaches a log line or an error text: the echo of each
//! command and every line a tool prints pass through [`Redact`], which puts the setting's name in
//! place of its value. The tasks talk to an [`Exec`], so tests swap the real processes for a
//! recording fake, and [`Process`] takes another `PATH` for tests with fake programs.

use std::ffi::OsString;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

/// The values to hide, each with the name shown instead (`<APPLE_SIGNING_IDENTITY>`).
#[derive(Debug, Clone, Default)]
pub struct Redact(Vec<(String, String)>);

impl Redact {
    /// Hides `value` behind `<name>`; an empty value is ignored.
    pub fn add(&mut self, name: &str, value: &str) {
        let value = value.trim();
        if value.is_empty() {
            return;
        }
        self.0.push((value.to_owned(), format!("<{name}>")));
        // Longest first, so a value that contains another one is replaced whole.
        self.0.sort_by_key(|(v, _)| std::cmp::Reverse(v.len()));
    }

    /// `text` with every hidden value replaced by its name.
    pub fn apply(&self, text: &str) -> String {
        let mut out = text.to_owned();
        for (value, name) in &self.0 {
            out = out.replace(value.as_str(), name);
        }
        out
    }
}

/// One command: program, arguments, folder and the variables it sets or removes.
#[derive(Debug, Clone)]
pub struct Cmd {
    pub program: String,
    pub args: Vec<String>,
    pub dir: PathBuf,
    pub env: Vec<(String, String)>,
    pub env_remove: Vec<String>,
}

impl Cmd {
    /// `program` run in `dir`.
    pub fn new(program: &str, dir: &Path) -> Cmd {
        Cmd {
            program: program.to_owned(),
            args: Vec::new(),
            dir: dir.to_path_buf(),
            env: Vec::new(),
            env_remove: Vec::new(),
        }
    }

    /// Adds arguments.
    pub fn args<S: AsRef<str>>(mut self, args: impl IntoIterator<Item = S>) -> Cmd {
        self.args
            .extend(args.into_iter().map(|a| a.as_ref().to_owned()));
        self
    }

    /// Adds one path argument.
    pub fn path(mut self, p: &Path) -> Cmd {
        self.args.push(p.to_string_lossy().into_owned());
        self
    }

    /// Sets a variable for this command only.
    pub fn env(mut self, name: &str, value: &str) -> Cmd {
        self.env.push((name.to_owned(), value.to_owned()));
        self
    }

    /// Removes a variable for this command only.
    pub fn env_remove(mut self, name: &str) -> Cmd {
        self.env_remove.push(name.to_owned());
        self
    }

    /// The command as a log shows it: hidden values replaced, the names (never the values) of
    /// the variables it sets.
    pub fn shown(&self, redact: &Redact) -> String {
        let mut line = format!("$ {} {}", self.program, self.args.join(" "));
        if !self.env.is_empty() {
            let names: Vec<&str> = self.env.iter().map(|(n, _)| n.as_str()).collect();
            line.push_str(&format!(" (sets {})", names.join(", ")));
        }
        redact.apply(line.trim_end())
    }
}

/// What a finished command printed (already redacted) and whether it succeeded.
#[derive(Debug, Clone, Default)]
pub struct Ran {
    pub ok: bool,
    pub status: String,
    pub out: String,
    pub err: String,
}

/// Runs commands; the one seam between the release tasks and the processes.
pub trait Exec {
    /// Runs `cmd` to its end. `Err` only when the program cannot be started.
    fn run(&mut self, cmd: &Cmd) -> Result<Ran, String>;
    /// The values this runner hides.
    fn redact(&self) -> &Redact;
    /// Prints one line of the task's own (it is redacted too).
    fn say(&mut self, line: &str);
}

/// Runs `cmd` and fails, naming the command (redacted), when it does not succeed.
pub fn check(exec: &mut dyn Exec, cmd: &Cmd) -> Result<Ran, String> {
    let ran = exec.run(cmd)?;
    if ran.ok {
        return Ok(ran);
    }
    let shown = cmd.shown(exec.redact());
    Err(format!(
        "`{}` exited with {}",
        shown.trim_start_matches("$ "),
        ran.status
    ))
}

/// Where printed lines go: the terminal, or a list a test reads.
type Sink = Option<Arc<Mutex<Vec<String>>>>;

/// The real runner: child processes with their output streamed line by line through [`Redact`].
#[derive(Debug, Default)]
pub struct Process {
    redact: Redact,
    path: Option<OsString>,
    sink: Sink,
}

impl Process {
    /// A runner that hides `redact`'s values.
    pub fn new(redact: Redact) -> Process {
        Process {
            redact,
            ..Process::default()
        }
    }

    /// Looks programs up in `path` instead of this process's `PATH` (tests with fake programs).
    #[cfg(test)]
    pub fn with_path(mut self, path: OsString) -> Process {
        self.path = Some(path);
        self
    }

    /// Collects every printed line in `sink` instead of the terminal (tests).
    #[cfg(test)]
    pub fn with_sink(mut self, sink: Arc<Mutex<Vec<String>>>) -> Process {
        self.sink = Some(sink);
        self
    }
}

fn emit(sink: &Sink, line: String, to_err: bool) {
    match sink {
        Some(s) => {
            if let Ok(mut lines) = s.lock() {
                lines.push(line);
            }
        }
        None if to_err => eprintln!("{line}"),
        None => println!("{line}"),
    }
}

/// Reads `from` to its end, prints each line redacted and returns them joined.
fn pump(from: impl Read, redact: Redact, sink: Sink, to_err: bool) -> String {
    let mut reader = BufReader::new(from);
    let mut all = String::new();
    let mut buf = Vec::new();
    while reader.read_until(b'\n', &mut buf).is_ok_and(|n| n > 0) {
        let raw = String::from_utf8_lossy(&buf);
        let line = redact.apply(raw.trim_end_matches(['\n', '\r']));
        all.push_str(&line);
        all.push('\n');
        emit(&sink, line, to_err);
        buf.clear();
    }
    all
}

impl Exec for Process {
    fn run(&mut self, cmd: &Cmd) -> Result<Ran, String> {
        emit(&self.sink, cmd.shown(&self.redact), true);
        let mut c = Command::new(&cmd.program);
        c.args(&cmd.args)
            .current_dir(&cmd.dir)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(p) = &self.path {
            c.env("PATH", p);
        }
        for (k, v) in &cmd.env {
            c.env(k, v);
        }
        for k in &cmd.env_remove {
            c.env_remove(k);
        }
        let mut child = c
            .spawn()
            .map_err(|e| format!("cannot run {}: {e}", cmd.program))?;
        let (out, err) = (child.stdout.take(), child.stderr.take());
        let (r1, s1) = (self.redact.clone(), self.sink.clone());
        let (r2, s2) = (self.redact.clone(), self.sink.clone());
        let t_out = std::thread::spawn(move || out.map(|o| pump(o, r1, s1, false)));
        let t_err = std::thread::spawn(move || err.map(|e| pump(e, r2, s2, true)));
        let status = child
            .wait()
            .map_err(|e| format!("{} did not finish: {e}", cmd.program))?;
        Ok(Ran {
            ok: status.success(),
            status: status.to_string(),
            out: t_out.join().ok().flatten().unwrap_or_default(),
            err: t_err.join().ok().flatten().unwrap_or_default(),
        })
    }

    fn redact(&self) -> &Redact {
        &self.redact
    }

    fn say(&mut self, line: &str) {
        emit(&self.sink, self.redact.apply(line), true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secrets() -> Redact {
        let mut r = Redact::default();
        r.add(
            "APPLE_SIGNING_IDENTITY",
            "Developer ID Application: Test Person (TEAM123456)",
        );
        r.add("APPLE_SIGNING_IDENTITY team", "TEAM123456");
        r.add("TOKNILA_NOTARY_PROFILE", "fake-notary-profile");
        r.add("EMPTY", "  ");
        r
    }

    #[test]
    fn a_value_is_replaced_by_its_name_and_the_longer_value_wins() {
        let r = secrets();
        assert_eq!(
            r.apply("origin=Developer ID Application: Test Person (TEAM123456) team TEAM123456"),
            "origin=<APPLE_SIGNING_IDENTITY> team <APPLE_SIGNING_IDENTITY team>"
        );
        assert_eq!(r.apply("nothing here"), "nothing here");
    }

    #[test]
    fn the_shown_command_names_its_variables_but_not_their_values() {
        let cmd = Cmd::new("xcrun", Path::new("."))
            .args(["notarytool", "submit", "a.dmg", "--keychain-profile"])
            .args(["fake-notary-profile"])
            .env("TAURI_SIGNING_PRIVATE_KEY_PASSWORD", "hunter2-password");
        let shown = cmd.shown(&secrets());
        assert_eq!(
            shown,
            "$ xcrun notarytool submit a.dmg --keychain-profile <TOKNILA_NOTARY_PROFILE> (sets TAURI_SIGNING_PRIVATE_KEY_PASSWORD)"
        );
        assert!(!shown.contains("hunter2"));
    }

    // `sh` is a unix program; the Windows path of this runner is the same code without it.
    #[cfg(unix)]
    #[test]
    fn a_child_s_output_is_redacted_line_by_line_and_a_failure_names_no_value() {
        let sink = Arc::new(Mutex::new(Vec::new()));
        let mut p = Process::new(secrets()).with_sink(sink.clone());
        let cmd = Cmd::new("sh", Path::new(".")).args([
            "-c",
            "echo 'origin=Developer ID Application: Test Person (TEAM123456)'; echo fake-notary-profile >&2; exit 3",
            "Developer ID Application: Test Person (TEAM123456)",
        ]);
        let err = check(&mut p, &cmd).expect_err("exit 3 fails");
        let lines = sink.lock().expect("not poisoned").clone();
        assert!(
            lines.contains(&"origin=<APPLE_SIGNING_IDENTITY>".to_owned()),
            "{lines:?}"
        );
        assert!(
            lines.contains(&"<TOKNILA_NOTARY_PROFILE>".to_owned()),
            "{lines:?}"
        );
        for text in lines.iter().chain([&err]) {
            assert!(!text.contains("Test Person"), "{text}");
            assert!(!text.contains("TEAM123456"), "{text}");
            assert!(!text.contains("fake-notary-profile"), "{text}");
        }
        assert!(err.ends_with("exited with exit status: 3"), "{err}");
    }
}
