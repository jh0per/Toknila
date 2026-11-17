//! The extra checks of a check turn: after the check command passed, every extra check of the
//! project whose paths match a file the ship changed against the synced main runs in the ship,
//! in the project's order, and each leaves a line of the round. The first failure stops them.

use std::time::Instant;

use toknila_core::{ExtraCheck, ShipLook, TicketId, toolpath};

use super::{CheckError, CheckResult, CheckSpec, CheckStage, report};
use crate::external::Host;
use crate::sync::{Seen, SyncSpec};

/// What the host saw while a check turn ran.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Ran {
    /// The main commit the ship was synced to and the ship's HEAD.
    pub seen: Option<Seen>,
    /// The extra checks that ran, in order.
    pub extras: Vec<ExtraRun>,
    /// The check command's failed first run, when its second run passed.
    pub first: Option<CheckResult>,
}

/// One extra check that ran.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtraRun {
    /// The check's paths as the round shows them.
    pub label: String,
    pub command: String,
    pub result: Result<CheckResult, CheckError>,
    /// How long it took.
    pub secs: u64,
    /// The failed first run, when the second one passed.
    pub first: Option<CheckResult>,
}

impl ExtraRun {
    /// Whether it ran and exited with success.
    pub fn passed(&self) -> bool {
        self.result.as_ref().is_ok_and(|r| r.success)
    }

    /// The round's line: `extra check apps/web/**: PASS cargo xtask web, 4 min`.
    pub fn line(&self) -> String {
        let verdict = if self.passed() { "PASS" } else { "FAIL" };
        format!(
            "extra check {}: {verdict} {}, {}",
            self.label,
            self.command,
            took(self.secs)
        )
    }

    /// What a worker reads about a failure, headline first: `extra check <paths>: `<command>`
    /// exited with code N.` and the end of its output.
    pub fn failure(&self) -> String {
        match &self.result {
            Ok(r) => format!("extra check {}: {}", self.label, report(&self.command, r)),
            Err(e) => format!("extra check {}: `{}` {e}.", self.label, self.command),
        }
    }
}

/// What the check command and the extra checks say together.
pub(super) struct Summary {
    /// The command and every extra check passed.
    pub passed: bool,
    /// What a worker reads: the command's report, then a line per extra check (a failing one
    /// with how it ended).
    pub output: String,
    /// The extra checks' lines.
    pub lines: Vec<String>,
    /// The first extra command whose program was not found on this machine.
    pub missing: Option<String>,
    /// The last 30 lines of every first run that failed before a second run passed.
    pub flaky: Option<String>,
}

/// Puts the check command's `result` and the `extras` that ran into one verdict.
pub(super) fn summarize(
    command: &str,
    (result, first): (&CheckResult, Option<&CheckResult>),
    extras: &[ExtraRun],
) -> Summary {
    let flaky: Vec<String> = first
        .map(|f| toknila_core::last_lines(&f.output))
        .into_iter()
        .chain(extras.iter().filter_map(|e| {
            let f = e.first.as_ref()?;
            Some(format!(
                "{}\n{}",
                e.line(),
                toknila_core::last_lines(&f.output)
            ))
        }))
        .collect();
    let flaky = (result.success && !flaky.is_empty()).then(|| flaky.join("\n\n"));
    let output = match extras
        .iter()
        .find(|e| !e.passed())
        .filter(|_| result.success)
    {
        // An extra check failed after the main command passed: the round opens with that check,
        // then says the main command passed, then the extra checks that passed.
        Some(failed) => {
            let mut output = format!("{}\n\n`{command}` passed.", failed.failure());
            for e in extras.iter().filter(|e| e.passed()) {
                output.push('\n');
                output.push_str(&e.line());
            }
            output
        }
        None => {
            let mut output = report(command, result);
            if !extras.is_empty() {
                output.push('\n');
                for e in extras {
                    output.push('\n');
                    output.push_str(&e.line());
                }
            }
            output
        }
    };
    let missing = extras.iter().find_map(|e| match &e.result {
        Ok(r) if !r.success && !r.timed_out => {
            let tail = super::tail(r.output.trim_end(), super::OUTPUT_LIMIT);
            toolpath::command_not_found(r.code, &tail).then(|| e.command.clone())
        }
        _ => None,
    });
    Summary {
        passed: result.success && extras.iter().all(ExtraRun::passed),
        output,
        lines: extras.iter().map(ExtraRun::line).collect(),
        missing,
        flaky,
    }
}

/// A duration as a person reads it: seconds under a minute, then minutes.
fn took(secs: u64) -> String {
    if secs < 60 {
        format!("{secs} s")
    } else {
        format!("{} min", (secs + 30) / 60)
    }
}

/// Runs the extra checks that match the files the ship changed against main, in order, while the
/// check command passed (`main_passed`); stops after the first that fails. A host that cannot
/// list the changes runs none.
pub(super) fn run_all(
    host: &Host<'_>,
    ticket: TicketId,
    spec: &CheckSpec,
    (sync, seen): (Option<&SyncSpec>, Option<&Seen>),
    extras: &[ExtraCheck],
) -> Vec<ExtraRun> {
    let Some(sync) = sync.filter(|_| !extras.is_empty()) else {
        return Vec::new();
    };
    let look = ShipLook {
        path: sync.path.clone(),
        start: sync.base.clone(),
    };
    let main = seen.map_or(sync.base.as_str(), |s| s.base.as_str());
    let Ok((_, files)) = host.yard.changes_between(&look, main, "HEAD") else {
        return Vec::new();
    };
    let mut ran = Vec::new();
    for extra in extras.iter().filter(|e| e.matches(&files)) {
        let command = extra.command.clone();
        host.progress.stage(
            ticket,
            CheckStage::Running {
                command: command.clone(),
            },
        );
        let started = Instant::now();
        let attempt = super::run_twice(
            host.checker,
            &CheckSpec {
                command: command.clone(),
                dir: spec.dir.clone(),
            },
        );
        let run = ExtraRun {
            label: extra.label(),
            command,
            result: attempt.result,
            secs: started.elapsed().as_secs(),
            first: attempt.first,
        };
        let failed = !run.passed();
        ran.push(run);
        if failed {
            break;
        }
    }
    ran
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(ok: bool, secs: u64) -> ExtraRun {
        ExtraRun {
            label: "apps/web/**".to_owned(),
            command: "cargo xtask web && cargo xtask e2e-web".to_owned(),
            result: Ok(CheckResult {
                success: ok,
                code: Some(i32::from(!ok)),
                timed_out: false,
                output: "boom".to_owned(),
            }),
            secs,
            first: None,
        }
    }

    #[test]
    fn a_line_says_the_paths_the_verdict_the_command_and_the_time() {
        assert_eq!(
            run(true, 240).line(),
            "extra check apps/web/**: PASS cargo xtask web && cargo xtask e2e-web, 4 min"
        );
        assert!(run(false, 5).line().ends_with(", 5 s"));
        assert!(run(false, 5).failure().contains("boom"));
    }

    fn main_result(ok: bool) -> CheckResult {
        CheckResult {
            success: ok,
            code: Some(i32::from(!ok)),
            timed_out: false,
            output: "main output".to_owned(),
        }
    }

    #[test]
    fn a_round_whose_extra_check_failed_opens_with_that_check_and_then_says_the_main_passed() {
        let sum = summarize(
            "cargo xtask verify",
            (&main_result(true), None),
            &[run(false, 5)],
        );
        assert!(!sum.passed);
        assert!(
            sum.output.starts_with(
                "extra check apps/web/**: `cargo xtask web && cargo xtask e2e-web` exited with code 1.\n\n```\nboom\n```"
            ),
            "{}",
            sum.output
        );
        assert!(
            sum.output.contains("\n\n`cargo xtask verify` passed."),
            "{}",
            sum.output
        );
        assert!(!sum.output.contains("exited with code 0"), "{}", sum.output);
    }

    #[test]
    fn a_round_whose_main_command_failed_reads_as_before() {
        let sum = summarize("cargo xtask verify", (&main_result(false), None), &[]);
        assert_eq!(
            sum.output,
            "`cargo xtask verify` exited with code 1.\n\n```\nmain output\n```"
        );
    }
}
