//! Patching a `tasks.md` from a chain: checkboxes are flipped to match subtask state and cards
//! that did not come from the file are appended. Every other byte stays; nothing is deleted.

use std::collections::HashSet;

use super::{SpecError, SpecPhase, SpecTask, is_task_id, parse, task_key, text_after_id};

/// A card of the chain, as far as export needs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportCard {
    pub title: String,
    pub prompt: String,
    pub subtasks: Vec<ExportSubtask>,
}

/// A subtask of a card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportSubtask {
    pub text: String,
    pub done: bool,
}

/// Returns `text` with checkboxes matched to `cards` and the missing cards appended. A task is
/// matched by its T-id (or its whole text without one); a card by its tasks, else by its title.
/// New phases and tasks follow the highest numbers in the file, with the file's line ending.
pub fn patch(text: &str, cards: &[ExportCard]) -> Result<String, SpecError> {
    let spec = parse(text)?;
    let mut out = text.to_owned();
    let mut fresh: Vec<&ExportCard> = Vec::new();
    for card in cards {
        match find_phase(&spec.phases, card) {
            Some(phase) => flip(&mut out, phase, card),
            None => fresh.push(card),
        }
    }
    if !fresh.is_empty() {
        append(&mut out, &spec.phases, &fresh);
    }
    Ok(out)
}

/// The phase a card came from: one that holds a task with the same key, else the same title.
fn find_phase<'a>(phases: &'a [SpecPhase], card: &ExportCard) -> Option<&'a SpecPhase> {
    let keys: HashSet<&str> = card.subtasks.iter().map(|s| task_key(&s.text)).collect();
    phases
        .iter()
        .find(|p| p.tasks.iter().any(|t| keys.contains(task_key(&t.text))))
        .or_else(|| phases.iter().find(|p| p.title == card.title))
}

/// Flips the checkbox of every task of `phase` whose subtask says otherwise. A mark is one
/// ASCII byte, so offsets stay valid.
fn flip(out: &mut String, phase: &SpecPhase, card: &ExportCard) {
    let mut used = vec![false; phase.tasks.len()];
    for sub in &card.subtasks {
        let Some(i) = match_task(&phase.tasks, &used, &sub.text) else {
            continue;
        };
        used[i] = true;
        let task = &phase.tasks[i];
        if task.done != sub.done {
            let mark = if sub.done { "x" } else { " " };
            out.replace_range(task.mark_offset..=task.mark_offset, mark);
        }
    }
}

/// The first unused task with the subtask's key, else (for a subtask without a T-id, as written
/// by an earlier export) with the same text after its id.
fn match_task(tasks: &[SpecTask], used: &[bool], text: &str) -> Option<usize> {
    let free = |i: &usize| !used[*i];
    let key = task_key(text);
    (0..tasks.len())
        .filter(free)
        .find(|i| task_key(&tasks[*i].text) == key)
        .or_else(|| {
            (0..tasks.len())
                .filter(free)
                .find(|i| text_after_id(&tasks[*i].text) == text.trim())
        })
}

fn append(out: &mut String, phases: &[SpecPhase], fresh: &[&ExportCard]) {
    let eol = if out.contains("\r\n") { "\r\n" } else { "\n" };
    let mut next_task = highest_task_id(phases) + 1;
    let width = highest_task_id(phases).to_string().len().max(3);
    let first_phase = phases.iter().filter_map(|p| p.number).max().unwrap_or(0) + 1;
    if !out.is_empty() && !out.ends_with('\n') {
        out.push_str(eol);
    }
    for (next_phase, card) in (first_phase..).zip(fresh) {
        let blank_before = out.is_empty() || out.ends_with("\n\n") || out.ends_with("\r\n\r\n");
        if !blank_before {
            out.push_str(eol);
        }
        let title = one_line(&card.title);
        out.push_str(&format!("## Phase {next_phase}: {title}{eol}"));
        let prompt = card.prompt.trim();
        if !prompt.is_empty() {
            out.push_str(eol);
            for line in prompt.lines() {
                out.push_str(line.trim_end());
                out.push_str(eol);
            }
        }
        if !card.subtasks.is_empty() {
            out.push_str(eol);
        }
        for sub in &card.subtasks {
            let mark = if sub.done { 'x' } else { ' ' };
            let body = one_line(&sub.text);
            let first = body.split_whitespace().next().unwrap_or_default();
            if is_task_id(first) {
                out.push_str(&format!("- [{mark}] {body}{eol}"));
            } else {
                let id = format!("T{next_task:0width$}");
                next_task += 1;
                out.push_str(&format!("- [{mark}] {id} {body}{eol}"));
            }
        }
    }
}

/// The largest `T<number>` among the tasks of the file.
fn highest_task_id(phases: &[SpecPhase]) -> u64 {
    phases
        .iter()
        .flat_map(|p| &p.tasks)
        .filter_map(|t| {
            let first = t.text.split_whitespace().next()?;
            is_task_id(first).then(|| first[1..].parse::<u64>().ok())?
        })
        .max()
        .unwrap_or(0)
}

fn one_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}
