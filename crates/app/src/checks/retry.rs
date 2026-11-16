//! A check that fails runs once more on the same commit, right away, while it still holds its
//! turn in the machine's line. A pass on the second run is a flaky test, not a round of the loop.

use toknila_core::toolpath;

use super::{CheckError, CheckResult, CheckSpec, Checker, OUTPUT_LIMIT, tail};

/// What running a command up to twice came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attempt {
    /// How the last run ended; the one a round reports.
    pub result: Result<CheckResult, CheckError>,
    /// The failed first run, when the second one passed.
    pub first: Option<CheckResult>,
}

/// Whether a failed run is the machine's problem: the program was not found.
pub fn cannot_run(r: &CheckResult) -> bool {
    !r.success
        && !r.timed_out
        && toolpath::command_not_found(r.code, &tail(r.output.trim_end(), OUTPUT_LIMIT))
}

/// Runs `spec`; when it ran and failed (not a timeout, and not a program that was not found),
/// runs it once more.
pub fn run_twice(checker: &dyn Checker, spec: &CheckSpec) -> Attempt {
    match checker.run(spec) {
        Ok(first) if !first.success && !first.timed_out && !cannot_run(&first) => {
            let second = checker.run(spec);
            let passed = second.as_ref().is_ok_and(|r| r.success);
            Attempt {
                result: second,
                first: passed.then_some(first),
            }
        }
        other => Attempt {
            result: other,
            first: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;

    #[derive(Debug)]
    struct Script(RefCell<Vec<(i32, &'static str)>>, RefCell<u32>);

    impl Checker for Script {
        fn run(&self, _: &CheckSpec) -> Result<CheckResult, CheckError> {
            *self.1.borrow_mut() += 1;
            let (code, output) = self.0.borrow_mut().remove(0);
            Ok(CheckResult {
                success: code == 0,
                code: Some(code),
                timed_out: false,
                output: output.to_owned(),
            })
        }
    }

    fn spec() -> CheckSpec {
        CheckSpec {
            command: "c".to_owned(),
            dir: ".".to_owned(),
        }
    }

    fn attempt(runs: Vec<(i32, &'static str)>) -> (Attempt, u32) {
        let s = Script(RefCell::new(runs), RefCell::new(0));
        let a = run_twice(&s, &spec());
        (a, *s.1.borrow())
    }

    #[test]
    fn fail_then_pass_is_flaky_and_two_failures_report_the_second() {
        let (a, n) = attempt(vec![(1, "first"), (0, "second")]);
        assert_eq!(n, 2);
        assert!(a.result.as_ref().is_ok_and(|r| r.success));
        assert_eq!(a.first.expect("first").output, "first");
        let (a, n) = attempt(vec![(1, "first"), (1, "second")]);
        assert_eq!(n, 2);
        assert_eq!(a.result.expect("ran").output, "second");
        assert!(a.first.is_none());
    }

    #[test]
    fn a_pass_and_a_missing_program_run_once() {
        assert_eq!(attempt(vec![(0, "ok")]).1, 1);
        assert_eq!(attempt(vec![(127, "sh: c: command not found")]).1, 1);
    }
}
