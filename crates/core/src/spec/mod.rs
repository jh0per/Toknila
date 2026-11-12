//! Spec folders: a `tasks.md` (as written by spec-driven toolkits) becomes a chain, and a chain
//! patches the same file back. Parsing, the dependency rule and patching are pure; reading and
//! writing files is the CLI's job.
//!
//! Mapping: one folder is one chain; one `## Phase N: ...` section with tasks is one ticket; one
//! `- [ ] T005 ...` line is one subtask whose text (T-id included) is kept verbatim. Phases that
//! name a user story run in parallel, each after the last other phase before them; any other
//! phase waits for everything since the previous one. Tasks before the first `##` heading are one
//! card titled after the spec. A card whose tasks are all checked but which sits behind a card
//! that is not is imported as done out of order (see [`OutOfOrderCard`]). Import goes through a chain file
//! ([`import`]); export only flips checkboxes and appends missing phases ([`patch`]).

mod deps;
mod import;
mod parse;
mod patch;
mod report;

pub use deps::phase_deps;
pub use import::{SpecImport, documents, import, markdown_title};
pub use parse::{SpecPhase, SpecTask, SpecTasks, parse};
pub use patch::{ExportCard, ExportSubtask, patch};
pub use report::{OpenBefore, OutOfOrderCard};

/// What is wrong with a `tasks.md`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SpecErrorKind {
    /// A checkbox line that is not `- [ ] text` or `- [x] text`.
    #[error("malformed task line: expected `- [ ] text` or `- [x] text`")]
    MalformedTask,
    /// A task line under a `#` heading that comes after the phases.
    #[error("task outside any phase")]
    TaskOutsidePhase,
    /// No phase in the file has a task.
    #[error("no phase with tasks")]
    NoTasks,
    /// The folder name cannot name a chain.
    #[error("empty epic name")]
    EmptyName,
}

/// A problem in a `tasks.md`. `line` is 1-based, 0 when it concerns the whole file.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("line {line}: {kind}")]
pub struct SpecError {
    pub line: usize,
    pub kind: SpecErrorKind,
}

impl SpecError {
    pub(crate) fn at(line: usize, kind: SpecErrorKind) -> Self {
        Self { line, kind }
    }
}

/// The key a task is matched by: its leading `T<number>` id, else its whole text.
pub fn task_key(text: &str) -> &str {
    match text.split_whitespace().next() {
        Some(first) if is_task_id(first) => first,
        _ => text.trim(),
    }
}

/// Task text without its leading id.
pub(crate) fn text_after_id(text: &str) -> &str {
    let text = text.trim();
    match text.split_once(char::is_whitespace) {
        Some((first, rest)) if is_task_id(first) => rest.trim_start(),
        _ => text,
    }
}

pub(crate) fn is_task_id(s: &str) -> bool {
    s.len() > 1 && s.starts_with('T') && s[1..].bytes().all(|b| b.is_ascii_digit())
}

#[cfg(test)]
mod tests;
