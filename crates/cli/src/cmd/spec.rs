//! Spec folders for `tok chain import|export`: reads and writes the files, the pure rules live in
//! `toknila_core::spec`.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Serialize;
use toknila_client::Client;
use toknila_core::spec::{self, ExportCard, ExportSubtask};
use toknila_core::{NodeRef, TicketStatus};

use super::{default_project, emit};

const TASKS: &str = "tasks.md";

/// What importing one folder did.
#[derive(Debug, Serialize)]
struct Outcome {
    folder: String,
    /// `imported`, `skipped` (no tasks.md) or `failed`.
    result: &'static str,
    cards: usize,
    tasks: usize,
    /// Cards imported as Merged because every task was checked.
    done_cards: usize,
    /// Done cards that sit behind a card with open subtasks, one line each.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    out_of_order: Vec<String>,
    /// The reason, for `skipped` and `failed`.
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

impl Outcome {
    fn new(folder: &Path, result: &'static str) -> Self {
        Self {
            folder: folder_name(folder),
            result,
            cards: 0,
            tasks: 0,
            done_cards: 0,
            out_of_order: Vec::new(),
            error: None,
        }
    }

    fn line(&self) -> String {
        match (&self.error, self.result) {
            (Some(e), r) => format!("{}: {r}: {e}", self.folder),
            (None, _) => {
                let head = format!(
                    "{}: imported, {} cards ({} done), {} tasks",
                    self.folder, self.cards, self.done_cards, self.tasks
                );
                self.out_of_order
                    .iter()
                    .fold(head, |out, l| format!("{out}\n  {l}"))
            }
        }
    }
}

/// Imports one spec folder, or every spec folder inside a parent folder, as chains. Reports each
/// folder and goes on after a failure; the command fails when any folder did.
pub(super) fn import(
    c: &Client,
    dir: &Path,
    project: Option<String>,
    json_out: bool,
) -> Result<()> {
    let folders = if dir.join(TASKS).is_file() {
        vec![dir.to_path_buf()]
    } else {
        subfolders(dir)?
    };
    if folders.is_empty() {
        bail!("no subfolders in {}", dir.display());
    }
    let project = default_project(c, project)?;
    let outcomes: Vec<Outcome> = folders
        .iter()
        .map(|f| import_folder(c, f, &project))
        .collect();
    emit(json_out, &outcomes, |all| {
        all.iter().for_each(|o| println!("{}", o.line()));
        let n = |r: &str| all.iter().filter(|o| o.result == r).count();
        println!(
            "{} imported, {} skipped, {} failed",
            n("imported"),
            n("skipped"),
            n("failed")
        );
    });
    let failed = outcomes.iter().filter(|o| o.result == "failed").count();
    if failed > 0 {
        bail!("{failed} of {} folders failed to import", outcomes.len());
    }
    Ok(())
}

fn import_folder(c: &Client, dir: &Path, project: &str) -> Outcome {
    let tasks = dir.join(TASKS);
    let text = match std::fs::read(&tasks) {
        Ok(bytes) => String::from_utf8(bytes).map_err(|_| "not UTF-8 text".to_owned()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let mut o = Outcome::new(dir, "skipped");
            o.error = Some(format!("no {TASKS}"));
            return o;
        }
        Err(e) => Err(e.to_string()),
    };
    let fail = |message: String| {
        let mut o = Outcome::new(dir, "failed");
        o.error = Some(message);
        o
    };
    let text = match text {
        Ok(t) => t,
        Err(e) => return fail(format!("{}: {e}", tasks.display())),
    };
    let title = std::fs::read_to_string(dir.join("spec.md"))
        .ok()
        .and_then(|t| spec::markdown_title(&t));
    let imp = match spec::import(&text, &folder_name(dir), title.as_deref(), &documents(dir)) {
        Ok(i) => i,
        Err(e) => return fail(format!("{}:{}: {}", tasks.display(), e.line, e.kind)),
    };
    if let Err(e) = c.import_chain(project, imp.toml) {
        return fail(e.to_string());
    }
    let mut o = Outcome::new(dir, "imported");
    (o.cards, o.tasks, o.done_cards) = (imp.cards, imp.tasks, imp.done_cards);
    o.out_of_order = imp.out_of_order.iter().map(ToString::to_string).collect();
    o
}

/// Patches `<folder>/tasks.md` from the chain: checkboxes follow the subtasks, cards that are
/// not in the file are appended. The file is only written when something changed.
pub(super) fn export(c: &Client, chain: &str, dir: &Path) -> Result<()> {
    let path = dir.join(TASKS);
    let bytes = std::fs::read(&path).with_context(|| format!("read {}", path.display()))?;
    let text =
        String::from_utf8(bytes).with_context(|| format!("{} is not UTF-8", path.display()))?;
    let detail = c.chain(chain)?;
    let mut cards = Vec::new();
    for node in &detail.order {
        let NodeRef::Ticket(id) = node else { continue };
        let Some(t) = detail.tickets.iter().find(|t| t.id == *id) else {
            continue;
        };
        // A card still awaiting approval is not part of the plan yet; a rejected one never was.
        if matches!(t.status, TicketStatus::Proposed | TicketStatus::Cancelled) {
            continue;
        }
        let mut subtasks = c.ticket(&t.key.to_string())?.subtasks;
        subtasks.sort_by_key(|s| s.position);
        cards.push(ExportCard {
            title: t.title.clone(),
            prompt: t.prompt.clone(),
            subtasks: subtasks
                .into_iter()
                .map(|s| ExportSubtask {
                    text: s.text,
                    done: s.done,
                })
                .collect(),
        });
    }
    let out = spec::patch(&text, &cards)
        .map_err(|e| anyhow::anyhow!("{}:{}: {}", path.display(), e.line, e.kind))?;
    if out == text {
        eprintln!("{} is up to date", path.display());
    } else {
        std::fs::write(&path, out).with_context(|| format!("write {}", path.display()))?;
        eprintln!("updated {}", path.display());
    }
    Ok(())
}

/// The chain name: the folder's own name.
fn folder_name(dir: &Path) -> String {
    let name = |p: &Path| p.file_name().map(|n| n.to_string_lossy().into_owned());
    name(dir)
        .or_else(|| dir.canonicalize().ok().and_then(|p| name(&p)))
        .unwrap_or_default()
}

/// Folders directly inside `dir`, sorted by name.
fn subfolders(dir: &Path) -> Result<Vec<PathBuf>> {
    let entries = std::fs::read_dir(dir).with_context(|| format!("read {}", dir.display()))?;
    let mut out: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir())
        .filter(|p| !folder_name(p).starts_with('.'))
        .collect();
    out.sort();
    Ok(out)
}

/// The folder's documents by relative path: the usual ones first, in their usual order, then the
/// rest by name; folders end in `/`.
fn documents(dir: &Path) -> Vec<String> {
    const USUAL: [&str; 7] = [
        "spec.md",
        "plan.md",
        "research.md",
        "data-model.md",
        "contracts/",
        "quickstart.md",
        "tasks.md",
    ];
    let mut found: Vec<String> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            if e.path().is_dir() {
                format!("{name}/")
            } else {
                name
            }
        })
        .filter(|n| !n.starts_with('.'))
        .collect();
    found.sort();
    let rank = |n: &String| USUAL.iter().position(|u| u == n).unwrap_or(USUAL.len());
    found.sort_by_key(rank);
    found
}
