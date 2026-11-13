//! The story's checklist as the worker reads it, so `subtask_done` names a position it has seen.

use super::RunBrief;
use crate::{Actor, Subtask};

/// One task of the story's checklist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BriefTask {
    pub position: u32,
    pub text: String,
    pub done: bool,
    /// Who ticked it, when done.
    pub done_by: Option<Actor>,
}

impl BriefTask {
    /// The tasks of a card in checklist order.
    pub fn of(subtasks: &[Subtask]) -> Vec<Self> {
        let mut out: Vec<Self> = subtasks
            .iter()
            .map(|s| Self {
                position: s.position,
                text: s.text.clone(),
                done: s.done,
                done_by: s.done_by,
            })
            .collect();
        out.sort_by_key(|t| t.position);
        out
    }

    fn line(&self) -> String {
        let mark = if self.done { "x" } else { " " };
        // Only a person's tick is worth saying: the agent's own is what it did itself.
        let by = match (self.done, self.done_by) {
            (true, Some(Actor::Human)) => " (done by a person)",
            _ => "",
        };
        format!(
            "{}. [{mark}] {}{by}\n",
            self.position,
            self.text.trim().replace('\n', " ")
        )
    }
}

/// The section with the checklist, after the acceptance; empty when the story has none.
pub(super) fn section(brief: &RunBrief<'_>) -> String {
    if brief.checklist.is_empty() {
        return String::new();
    }
    let mut out = String::from(
        "\n## Checklist\n\nThe story's tasks, by position. Tick a task with `subtask_done` \
         (its position) when you did it; never ask a person whether to tick one.\n\n",
    );
    for task in brief.checklist {
        out.push_str(&task.line());
    }
    out
}
