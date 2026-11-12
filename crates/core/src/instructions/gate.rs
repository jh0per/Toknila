//! The gate's result as the reviewer is told it: what Toknila's own run of the check command said
//! about the hand-in, or that Toknila did not run it.

use crate::CheckOutcome;

/// The block that carries the gate's result, for the reviewer's kickoff and instructions file.
pub(super) fn text(check: Option<&CheckOutcome>) -> String {
    let Some(c) = check else {
        return "Toknila did not run the gate on this hand-in (the project has no check command, or \
                it did not run). There is no gate result to quote: say `NOT_RUN` and why for the \
                check item."
            .to_owned();
    };
    let word = if c.passed { "PASS" } else { "FAIL" };
    let command = if c.command.is_empty() {
        "the check command"
    } else {
        c.command.as_str()
    };
    format!(
        "Toknila ran the gate on this hand-in's commit: `{command}` finished {} with {word}. Quote \
         it as the evidence of the check item with basis `ran`; do not run the whole gate again. \
         Its output (the command and the end of its output):\n\n```\n{}\n```",
        c.at.utc_text(),
        c.output.trim_end()
    )
}
