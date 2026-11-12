//! The test step of `cargo xtask verify` and the timing file it leaves behind.
//!
//! Tests run under `cargo nextest` (one process per test, binaries in parallel) when it is
//! installed, else under `cargo test`; the step says which one it used. Doc tests always run under
//! `cargo test`, since nextest does not run them.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use crate::leaks::ROOT_VAR;

/// Is `cargo nextest` installed?
fn has_nextest(cargo: &str) -> bool {
    Command::new(cargo)
        .args(["nextest", "--version"])
        .output()
        .is_ok_and(|o| o.status.success())
}

fn run(cargo: &str, root: &Path, args: &[&str]) -> Result<(), String> {
    eprintln!("$ cargo {} ({ROOT_VAR}={})", args.join(" "), root.display());
    let status = Command::new(cargo)
        .args(args)
        .env(ROOT_VAR, root)
        .status()
        .map_err(|e| format!("cannot run cargo: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("`cargo {}` exited with {status}", args.join(" ")))
    }
}

/// The workspace tests with the test homes created under `root`.
pub fn cargo_test(cargo: &str, root: &Path) -> Result<(), String> {
    if has_nextest(cargo) {
        eprintln!("xtask verify: tests run with cargo nextest");
        // The tests of other crates start this binary; build it once, not once per test process.
        let build = [
            "build",
            "--locked",
            "-p",
            "toknila-daemon",
            "--bin",
            "fake-agent",
        ];
        run(cargo, root, &build)?;
        run(
            cargo,
            root,
            &[
                "nextest",
                "run",
                "--workspace",
                "--locked",
                "--no-fail-fast",
            ],
        )?;
        run(cargo, root, &["test", "--workspace", "--locked", "--doc"])
    } else {
        eprintln!("xtask verify: tests run with cargo test (cargo nextest is not installed)");
        run(cargo, root, &["test", "--workspace", "--locked"])
    }
}

/// How long each step of `verify` took, in seconds.
#[derive(Default)]
pub struct Timings(Vec<(&'static str, f64)>);

impl Timings {
    /// Runs one step and notes how long it took, whether it passed or not.
    pub fn phase<T>(&mut self, name: &'static str, f: impl FnOnce() -> T) -> T {
        let start = Instant::now();
        let out = f();
        self.0.push((name, start.elapsed().as_secs_f64()));
        out
    }

    /// Prints the steps and writes them to `<target>/verify-timing.json` (CI uploads it).
    pub fn write(&self, target: &Path) {
        let body: Vec<String> = self
            .0
            .iter()
            .map(|(n, s)| format!("  \"{n}\": {s:.1}"))
            .collect();
        for (n, s) in &self.0 {
            eprintln!("xtask verify: {n} took {s:.1}s");
        }
        let json = format!("{{\n{}\n}}\n", body.join(",\n"));
        let _ = std::fs::create_dir_all(target);
        let _ = std::fs::write(target.join("verify-timing.json"), json);
    }
}

/// The cargo target folder.
pub fn target_dir(root: &Path) -> PathBuf {
    std::env::var_os("CARGO_TARGET_DIR").map_or_else(|| root.join("target"), PathBuf::from)
}
