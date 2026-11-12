//! Reading a path for an import: a TOML chain file, a spec folder (one with `tasks.md`), or a
//! folder of spec folders. These are the rules `tok chain import` had, moved into the daemon so
//! the window can import by path too; the CLI now sends the path.

use std::path::{Path, PathBuf};

use toknila_api_types::FolderNote;
use toknila_app::AppError;
use toknila_core::plans::{Counts, SPEC_FILE, TASKS_FILE};
use toknila_core::spec;

use super::{canonical, name_of, text};

/// A chain file ready for the ordinary chain import, and what it holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prepared {
    pub toml: String,
    pub counts: Counts,
    /// Done cards behind cards with open tasks, one line each.
    pub out_of_order: Vec<String>,
}

/// One chain to import: where it is read from, and its chain file or why there is none (the
/// message names the file and, for a `tasks.md`, the line).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    /// The spec folder's or the chain file's name.
    pub folder: String,
    /// Resolved absolute path; the chain keeps it as its source.
    pub path: String,
    pub chain: Result<Prepared, String>,
}

/// What a path holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Found {
    /// A chain file or one spec folder.
    One(Candidate),
    /// A folder of spec folders: the spec folders by name, and the subfolders without `tasks.md`.
    Many {
        spec_folders: Vec<Candidate>,
        skipped: Vec<FolderNote>,
    },
}

/// Reads `path` for an import. A relative path, one that cannot be read, or a folder with neither
/// a `tasks.md` nor a spec folder inside is a bad request whose message names the path.
pub fn read(path: &str) -> Result<Found, AppError> {
    let given = path.trim();
    if given.is_empty() {
        return Err(AppError::bad_request(
            "give the absolute path of an epic file, a spec folder or a folder of spec folders",
        ));
    }
    let raw = Path::new(given);
    if !raw.is_absolute() {
        return Err(AppError::bad_request(format!(
            "{given} is not an absolute path"
        )));
    }
    let meta = std::fs::metadata(raw).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => AppError::bad_request(format!("{given} does not exist")),
        _ => AppError::bad_request(format!("cannot read {given}: {e}")),
    })?;
    let path = canonical(raw);
    if !meta.is_dir() {
        return Ok(Found::One(chain_file(&path)));
    }
    if path.join(TASKS_FILE).is_file() {
        return Ok(Found::One(spec_folder(&path)));
    }
    let (with, without): (Vec<PathBuf>, Vec<PathBuf>) = subfolders(&path, given)?
        .into_iter()
        .partition(|d| d.join(TASKS_FILE).is_file());
    if with.is_empty() {
        return Err(AppError::bad_request(format!(
            "{given} has no {TASKS_FILE}, and no folder in it has one"
        )));
    }
    let skipped = without.iter().map(|d| FolderNote {
        folder: name_of(d),
        path: text(d),
        reason: format!("no {TASKS_FILE}"),
    });
    Ok(Found::Many {
        spec_folders: with.iter().map(|d| spec_folder(d)).collect(),
        skipped: skipped.collect(),
    })
}

fn chain_file(path: &Path) -> Candidate {
    let chain = read_text(path).and_then(|toml| {
        let plan =
            toknila_core::plan::parse(&toml).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(Prepared {
            counts: Counts::of_chain_file(&plan),
            toml,
            out_of_order: Vec::new(),
        })
    });
    Candidate {
        folder: name_of(path),
        path: text(path),
        chain,
    }
}

fn spec_folder(dir: &Path) -> Candidate {
    let tasks = dir.join(TASKS_FILE);
    let name = name_of(dir);
    let chain = read_text(&tasks).and_then(|text| {
        let title = std::fs::read_to_string(dir.join(SPEC_FILE))
            .ok()
            .and_then(|t| spec::markdown_title(&t));
        let imp = spec::import(&text, &name, title.as_deref(), &documents(dir))
            .map_err(|e| format!("{}:{}: {}", tasks.display(), e.line, e.kind))?;
        Ok(Prepared {
            counts: Counts {
                cards: imp.cards,
                tasks: imp.tasks,
                done_cards: imp.done_cards,
            },
            out_of_order: imp.out_of_order.iter().map(ToString::to_string).collect(),
            toml: imp.toml,
        })
    });
    Candidate {
        folder: name,
        path: text(dir),
        chain,
    }
}

/// A file's text, or why it cannot be had; the message names the file.
fn read_text(path: &Path) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    String::from_utf8(bytes).map_err(|_| format!("{}: not UTF-8 text", path.display()))
}

/// The visible folders directly inside `dir`, by name, each resolved.
fn subfolders(dir: &Path, given: &str) -> Result<Vec<PathBuf>, AppError> {
    let entries = std::fs::read_dir(dir)
        .map_err(|e| AppError::bad_request(format!("cannot read {given}: {e}")))?;
    let mut out: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir() && !name_of(p).starts_with('.'))
        .collect();
    out.sort();
    Ok(out.iter().map(|p| canonical(p)).collect())
}

/// The spec folder's documents for the chain's goal, in core's order.
fn documents(dir: &Path) -> Vec<String> {
    let names = std::fs::read_dir(dir)
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
        });
    spec::documents(names)
}
