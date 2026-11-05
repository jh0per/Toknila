//! A `tasks.md` as a chain file: the input of the ordinary chain import.

use serde::Serialize;

use super::{SpecError, SpecErrorKind, SpecPhase, parse, phase_deps};

/// The chain file for a spec folder, and what it holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecImport {
    pub toml: String,
    /// Phases with tasks, one card each.
    pub cards: usize,
    pub tasks: usize,
    /// Cards whose tasks are all checked.
    pub done_cards: usize,
}

#[derive(Serialize)]
struct File {
    chain: Chain,
    ticket: Vec<Ticket>,
}

#[derive(Serialize)]
struct Chain {
    name: String,
    goal: String,
    autostart: bool,
}

#[derive(Serialize)]
struct Ticket {
    id: String,
    title: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    prompt: String,
    subtasks: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    done_subtasks: Vec<usize>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    after: Vec<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    done: bool,
}

/// Turns the `tasks.md` of the folder `name` into a chain file. `docs` lists the folder's
/// documents by relative path; they go into the chain's goal so a person or agent can open them.
pub fn import(tasks_md: &str, name: &str, docs: &[String]) -> Result<SpecImport, SpecError> {
    if name.trim().is_empty() {
        return Err(SpecError::at(0, SpecErrorKind::EmptyName));
    }
    let spec = parse(tasks_md)?;
    let phases: Vec<&SpecPhase> = spec.phases.iter().filter(|p| !p.tasks.is_empty()).collect();
    if phases.is_empty() {
        return Err(SpecError::at(0, SpecErrorKind::NoTasks));
    }
    let stories: Vec<bool> = phases.iter().map(|p| p.story).collect();
    let deps = phase_deps(&stories);
    let id = |i: usize| format!("p{}", i + 1);
    let ticket: Vec<Ticket> = phases
        .iter()
        .zip(&deps)
        .enumerate()
        .map(|(i, (p, after))| Ticket {
            id: id(i),
            title: p.title.clone(),
            prompt: p.prompt.clone(),
            subtasks: p.tasks.iter().map(|t| t.text.clone()).collect(),
            done_subtasks: (1..=p.tasks.len())
                .filter(|n| p.tasks[n - 1].done)
                .collect(),
            after: after.iter().map(|d| id(*d)).collect(),
            done: p.tasks.iter().all(|t| t.done),
        })
        .collect();
    let file = File {
        chain: Chain {
            name: name.trim().to_owned(),
            goal: goal(spec.title.as_deref(), docs),
            autostart: false,
        },
        ticket,
    };
    Ok(SpecImport {
        cards: file.ticket.len(),
        tasks: phases.iter().map(|p| p.tasks.len()).sum(),
        done_cards: file.ticket.iter().filter(|t| t.done).count(),
        toml: toml::to_string(&file).expect("a chain file of strings and numbers serializes"),
    })
}

/// The tasks title, then the documents of the folder.
fn goal(title: Option<&str>, docs: &[String]) -> String {
    let mut out = title.unwrap_or_default().to_owned();
    if !docs.is_empty() {
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        out.push_str("Documents:");
        for d in docs {
            out.push_str("\n- ");
            out.push_str(d);
        }
    }
    out
}
