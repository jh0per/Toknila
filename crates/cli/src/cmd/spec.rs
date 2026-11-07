//! Imports by path for `tok chain import` (the daemon reads the chain file or the spec folders)
//! and spec folder export for `tok chain export --spec` (patching `tasks.md` here). The pure
//! rules live in `toknila_core::spec`.

use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::Serialize;
use toknila_api_types::{ChainDetail, FolderNote, ImportPathResult};
use toknila_client::{Client, ClientError};
use toknila_core::spec::{self, ExportCard, ExportSubtask};
use toknila_core::{NodeRef, TicketStatus};

use super::{default_project, emit};

const TASKS: &str = toknila_core::plans::TASKS_FILE;

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
    fn note(n: FolderNote, result: &'static str) -> Self {
        Self {
            folder: n.folder,
            result,
            cards: 0,
            tasks: 0,
            done_cards: 0,
            out_of_order: Vec::new(),
            error: Some(n.reason),
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

/// A path given on the command line made absolute, as the daemon takes it.
fn absolute(path: &Path) -> Result<String> {
    let abs = std::path::absolute(path).with_context(|| format!("resolve {}", path.display()))?;
    Ok(abs.to_string_lossy().into_owned())
}

/// Imports a chain file; the daemon reads it and the chain remembers it as its source.
pub(super) fn import_file(c: &Client, file: &Path, project: Option<String>) -> Result<ChainDetail> {
    let project = default_project(c, project)?;
    let result = c.import_path(&project, &absolute(file)?)?;
    match result.chains.into_iter().next() {
        Some(imported) => Ok(imported.chain),
        None => bail!("{} made no chain", file.display()),
    }
}

/// Imports one spec folder, or every spec folder inside a parent folder, as chains; the daemon
/// reads them. Reports each folder; the command fails when any folder did.
pub(super) fn import(
    c: &Client,
    dir: &Path,
    project: Option<String>,
    json_out: bool,
) -> Result<()> {
    let project = default_project(c, project)?;
    let path = absolute(dir)?;
    let result = match c.import_path(&project, &path) {
        Ok(r) => r,
        // A single folder that fails is reported like one of many.
        Err(ClientError::Api { body, .. }) => ImportPathResult {
            failed: vec![FolderNote {
                folder: dir
                    .file_name()
                    .map_or_else(|| path.clone(), |n| n.to_string_lossy().into_owned()),
                path,
                reason: body.error,
            }],
            ..ImportPathResult::default()
        },
        Err(e) => return Err(e.into()),
    };
    let imported = result.chains.into_iter().map(|i| Outcome {
        folder: i.folder,
        result: "imported",
        cards: i.cards,
        tasks: i.tasks,
        done_cards: i.done_cards,
        out_of_order: i.out_of_order,
        error: None,
    });
    let mut outcomes: Vec<Outcome> = imported
        .chain(
            result
                .skipped
                .into_iter()
                .map(|n| Outcome::note(n, "skipped")),
        )
        .chain(
            result
                .failed
                .into_iter()
                .map(|n| Outcome::note(n, "failed")),
        )
        .collect();
    outcomes.sort_by(|a, b| a.folder.cmp(&b.folder));
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
