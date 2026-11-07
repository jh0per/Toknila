//! A `tasks.md` as a chain file: the input of the ordinary chain import.

use serde::Serialize;

use super::report::{self, OutOfOrderCard};
use super::{SpecError, SpecErrorKind, SpecPhase, parse, phase_deps};
use crate::plan::out_of_order;

/// The chain file for a spec folder, and what it holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecImport {
    pub toml: String,
    /// Phases with tasks, one card each.
    pub cards: usize,
    pub tasks: usize,
    /// Cards whose tasks are all checked.
    pub done_cards: usize,
    /// Done cards that sit behind a card with open tasks; they are imported as Merged anyway.
    pub out_of_order: Vec<OutOfOrderCard>,
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

/// Turns the `tasks.md` of the folder `name` into a chain file. `spec_title` is the title of the
/// folder's `spec.md`; `docs` lists the folder's documents by relative path, which go into the
/// chain's goal so a person or agent can open them. Tasks before the first `##` heading become
/// one card titled after the tasks file, else the spec, else the folder.
pub fn import(
    tasks_md: &str,
    name: &str,
    spec_title: Option<&str>,
    docs: &[String],
) -> Result<SpecImport, SpecError> {
    if name.trim().is_empty() {
        return Err(SpecError::at(0, SpecErrorKind::EmptyName));
    }
    let spec = parse(tasks_md)?;
    let phases: Vec<&SpecPhase> = spec.phases.iter().filter(|p| !p.tasks.is_empty()).collect();
    if phases.is_empty() {
        return Err(SpecError::at(0, SpecErrorKind::NoTasks));
    }
    let fallback = [spec.title.as_deref(), spec_title]
        .into_iter()
        .flatten()
        .map(str::trim)
        .find(|t| !t.is_empty())
        .unwrap_or(name.trim());
    let titles: Vec<&str> = phases
        .iter()
        .map(|p| {
            if p.title.is_empty() {
                fallback
            } else {
                &p.title
            }
        })
        .collect();
    let stories: Vec<bool> = phases.iter().map(|p| p.story).collect();
    let deps = phase_deps(&stories);
    let done: Vec<bool> = phases
        .iter()
        .map(|p| p.tasks.iter().all(|t| t.done))
        .collect();
    let cards: Vec<(&SpecPhase, &str)> =
        phases.iter().copied().zip(titles.iter().copied()).collect();
    let out_of_order = report::build(&cards, &out_of_order(&done, &deps));
    let id = |i: usize| format!("p{}", i + 1);
    let ticket: Vec<Ticket> = cards
        .iter()
        .zip(&deps)
        .enumerate()
        .map(|(i, ((p, title), after))| Ticket {
            id: id(i),
            title: (*title).to_owned(),
            prompt: p.prompt.clone(),
            subtasks: p.tasks.iter().map(|t| t.text.clone()).collect(),
            done_subtasks: (1..=p.tasks.len())
                .filter(|n| p.tasks[n - 1].done)
                .collect(),
            after: after.iter().map(|d| id(*d)).collect(),
            done: done[i],
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
        out_of_order,
        toml: toml::to_string(&file).expect("a chain file of strings and numbers serializes"),
    })
}

/// The title of a markdown document: its first `# ` heading.
pub fn markdown_title(text: &str) -> Option<String> {
    text.lines()
        .map(|l| l.trim_start_matches('\u{feff}'))
        .find_map(|l| l.strip_prefix("# "))
        .map(|t| t.trim().to_owned())
        .filter(|t| !t.is_empty())
}

/// The documents of a spec folder in the order the chain's goal lists them: the usual ones first,
/// in their usual order, then the rest by name. `names` are the folder's entries by relative path,
/// folders ending in `/`; hidden ones are left out.
pub fn documents(names: impl IntoIterator<Item = String>) -> Vec<String> {
    const USUAL: [&str; 7] = [
        "spec.md",
        "plan.md",
        "research.md",
        "data-model.md",
        "contracts/",
        "quickstart.md",
        "tasks.md",
    ];
    let mut found: Vec<String> = names.into_iter().filter(|n| !n.starts_with('.')).collect();
    found.sort();
    let rank = |n: &String| USUAL.iter().position(|u| u == n).unwrap_or(USUAL.len());
    found.sort_by_key(rank);
    found
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
