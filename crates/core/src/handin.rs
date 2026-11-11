//! The hand-in: the structured summary an agent gives with `finished`.
//!
//! ```markdown
//! **What changed:** a few lines
//!
//! **Checks:**
//! - PASS cargo xtask verify: 412 tests passed
//! - NOT_RUN e2e-web: no browser on this machine
//!
//! **Left:** what is left or not covered
//! ```
//!
//! [`check`] refuses a summary without the checks section, under [`MIN_CHARS`], or with a PASS
//! whose evidence is under [`MIN_EVIDENCE`]; [`to_body`]
//! splits an accepted one into the fields of a report comment so a card can show each block.

use crate::CommentBody;

/// The shortest summary (trimmed, in characters) a hand-in may be.
pub const MIN_CHARS: usize = 40;

/// The words a check line starts with, after the dash.
pub const STATUSES: [&str; 4] = ["PASS", "FAIL", "NOT_RUN", "BLOCKED"];

/// The shape of a hand-in; the refusal carries it and the instructions name it.
pub const TEMPLATE: &str = "**What changed:** a few lines
**Checks:**
- PASS|FAIL|NOT_RUN|BLOCKED <check>: <one line of evidence>
**Left:** what is left or not covered";

/// Why a summary is not a hand-in.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HandInError {
    #[error(
        "the hand-in is too short ({len} of at least {} characters). Use this template:\n{}",
        MIN_CHARS,
        TEMPLATE
    )]
    TooShort { len: usize },
    #[error("the hand-in has no checks section. Use this template:\n{}", TEMPLATE)]
    NoChecks,
    #[error(
        "the checks section has no line that starts with PASS, FAIL, NOT_RUN or BLOCKED. Use this template:\n{}",
        TEMPLATE
    )]
    NoCheckLines,
    #[error(
        "rule: a PASS needs evidence of at least {} characters, the gate's own result line word for word; this one has {len}: \"{line}\". Use PASS only with a quoted result, else NOT_RUN or FAIL with the reason.",
        MIN_EVIDENCE
    )]
    ThinEvidence { len: usize, line: String },
}

/// The shortest evidence (trimmed, in characters) a PASS line may carry.
pub const MIN_EVIDENCE: usize = 20;

/// The evidence of a PASS line (the text after the check's colon), `None` for other lines.
fn pass_evidence(line: &str) -> Option<&str> {
    let line = line
        .trim_start()
        .trim_start_matches(['-', '*'])
        .trim_start();
    let rest = line.strip_prefix("PASS")?;
    if !(rest.is_empty() || rest.starts_with([' ', ':'])) {
        return None;
    }
    Some(rest.split_once(':').map_or("", |(_, e)| e.trim()))
}

/// The three blocks of a hand-in; a block that is not there is empty.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Sections {
    /// What changed.
    pub changed: String,
    /// The checks section, one line per check (`- PASS name: evidence`).
    pub checks: String,
    /// What is left or not covered.
    pub left: String,
    /// Whether the checks label was there.
    pub has_checks: bool,
}

#[derive(Clone, Copy)]
enum Block {
    Changed,
    Checks,
    Left,
}

/// `**Label:** rest` → (block, rest); labels are case-insensitive.
fn label_line(line: &str) -> Option<(Block, &str)> {
    let rest = line.trim_start().strip_prefix("**")?;
    let (label, rest) = rest.split_once(":**")?;
    let block = match label.trim().to_lowercase().as_str() {
        "what changed" | "changed" => Block::Changed,
        "checks" => Block::Checks,
        "left" | "left / not covered" | "what is left" | "not covered" => Block::Left,
        _ => return None,
    };
    Some((block, rest.trim()))
}

/// Splits a summary at its labels. Text before the first label counts as what changed.
pub fn sections(summary: &str) -> Sections {
    let mut out = Sections::default();
    let mut current = Block::Changed;
    for line in summary.lines() {
        let text = match label_line(line) {
            Some((block, rest)) => {
                current = block;
                out.has_checks |= matches!(block, Block::Checks);
                rest
            }
            None => line.trim_end(),
        };
        let slot = match current {
            Block::Changed => &mut out.changed,
            Block::Checks => &mut out.checks,
            Block::Left => &mut out.left,
        };
        if !text.trim().is_empty() {
            if !slot.is_empty() {
                slot.push('\n');
            }
            slot.push_str(text);
        }
    }
    out
}

/// Whether a line of the checks section is a check: `- PASS ...` (the dash is optional).
pub fn is_check_line(line: &str) -> bool {
    let line = line
        .trim_start()
        .trim_start_matches(['-', '*'])
        .trim_start();
    STATUSES.iter().any(|s| {
        line.strip_prefix(s)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with([' ', ':']))
    })
}

/// Refuses a summary that is not a hand-in.
pub fn check(summary: &str) -> Result<(), HandInError> {
    let len = summary.trim().chars().count();
    if len < MIN_CHARS {
        return Err(HandInError::TooShort { len });
    }
    let parts = sections(summary);
    if !parts.has_checks || parts.checks.trim().is_empty() {
        return Err(HandInError::NoChecks);
    }
    if !parts.checks.lines().any(is_check_line) {
        return Err(HandInError::NoCheckLines);
    }
    for line in parts.checks.lines() {
        if let Some(e) = pass_evidence(line) {
            let len = e.chars().count();
            if len < MIN_EVIDENCE {
                return Err(HandInError::ThinEvidence {
                    len,
                    line: line.trim().to_owned(),
                });
            }
        }
    }
    Ok(())
}

/// A person's plain words (`tok sim ... finish`) as a hand-in that [`check`] accepts: nothing ran.
/// Text that already has a checks section is kept as it is.
pub fn from_plain(text: &str) -> String {
    let text = text.trim();
    if sections(text).has_checks {
        return text.to_owned();
    }
    let text = if text.is_empty() {
        "handed in by hand"
    } else {
        text
    };
    format!(
        "**What changed:** {text}\n**Checks:**\n- NOT_RUN all: handed in by hand, no check was run\n**Left:** nothing is known to be left"
    )
}

/// The report comment of an accepted hand-in: the gist is the first line of what changed, the
/// blocks are `what`, `how_to_verify` (the checks) and `consequences` (what is left). `None`
/// when the text has no checks section (a report that is not a hand-in).
pub fn to_body(summary: &str) -> Option<CommentBody> {
    let parts = sections(summary);
    if !parts.has_checks {
        return None;
    }
    let gist = parts
        .changed
        .lines()
        .next()
        .unwrap_or("Handed in")
        .trim()
        .to_owned();
    let rest: Vec<&str> = parts.changed.lines().skip(1).collect();
    let some = |s: String| (!s.trim().is_empty()).then_some(s);
    Some(CommentBody {
        gist,
        what: some(rest.join("\n")),
        how_to_verify: some(parts.checks),
        consequences: some(parts.left),
        ..CommentBody::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = "**What changed:** the hand-in has a template\n**Checks:**\n- PASS cargo xtask verify: test result: ok. 412 passed\n- NOT_RUN e2e-web: no browser\n**Left:** nothing";

    #[test]
    fn a_complete_hand_in_is_accepted() {
        assert_eq!(check(GOOD), Ok(()));
    }

    #[test]
    fn a_hand_in_without_checks_is_refused_with_the_template() {
        let long = "**What changed:** a long enough description of the change that was made";
        let err = check(long).expect_err("no checks");
        assert_eq!(err, HandInError::NoChecks);
        assert!(err.to_string().contains(TEMPLATE));
    }

    #[test]
    fn a_pass_with_thin_evidence_is_refused_naming_the_rule() {
        let s = "**What changed:** a long enough description of the change\n**Checks:**\n- PASS cargo xtask verify: all green\n**Left:** nothing";
        let err = check(s).expect_err("thin evidence");
        assert_eq!(
            err,
            HandInError::ThinEvidence {
                len: 9,
                line: "- PASS cargo xtask verify: all green".to_owned()
            }
        );
        assert!(err.to_string().contains("at least 20 characters"));
        let ok = s.replace("all green", "test result: ok. 9 passed");
        assert_eq!(check(&ok), Ok(()));
        let fail = s.replace("PASS", "FAIL");
        assert_eq!(check(&fail), Ok(()));
    }

    #[test]
    fn a_short_hand_in_is_refused() {
        assert_eq!(check("done"), Err(HandInError::TooShort { len: 4 }));
    }

    #[test]
    fn a_checks_section_needs_a_status_line() {
        let s = "**What changed:** a long enough description of the change\n**Checks:**\nall fine, trust me";
        assert_eq!(check(s), Err(HandInError::NoCheckLines));
    }

    #[test]
    fn the_blocks_become_the_fields_of_a_report() {
        let body = to_body(GOOD).expect("a hand-in");
        assert_eq!(body.gist, "the hand-in has a template");
        assert_eq!(body.what, None);
        assert_eq!(
            body.how_to_verify.as_deref(),
            Some(
                "- PASS cargo xtask verify: test result: ok. 412 passed\n- NOT_RUN e2e-web: no browser"
            )
        );
        assert_eq!(body.consequences.as_deref(), Some("nothing"));
        assert_eq!(to_body("just text"), None);
    }

    #[test]
    fn plain_words_make_an_accepted_hand_in() {
        assert_eq!(check(&from_plain("done")), Ok(()));
    }
}
