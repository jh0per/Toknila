//! Finding the plans of a project's repository: the entries of `specs/` and `plans/`, one level
//! deep, never the whole tree. Core says which entries are plans and counts their cards.

use std::path::Path;

use toknila_core::plans::{self, Content, PlanDir, PlanScan, SPECIFY_DIR, WorkingCopyBehind};

use crate::git::Git;

use super::{canonical, text};

/// A plan file bigger than this is listed without a card count.
const MAX_BYTES: u64 = 1 << 20;

/// The plans of the repository at `repo`. `sources` are the paths the project's chains were
/// imported from. A project without a repository, or whose folder is gone, has none.
pub fn scan(repo: Option<&str>, sources: &[String]) -> PlanScan {
    let root = repo
        .map(str::trim)
        .filter(|r| Path::new(r).is_absolute())
        .map(|r| canonical(Path::new(r)))
        .filter(|r| r.is_dir());
    let Some(root) = root else {
        return PlanScan::default();
    };
    let mut found = Vec::new();
    for dir in PlanDir::ALL {
        let Ok(entries) = std::fs::read_dir(root.join(dir.name())) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let path = canonical(&entry.path());
            let is_dir = path.is_dir();
            if !plans::wanted(dir, &name, is_dir) {
                continue;
            }
            let file = dir.plan_file().map_or(path.clone(), |f| path.join(f));
            let content = content(&file);
            found.extend(plans::found(
                dir,
                &name,
                &text(&path),
                is_dir,
                &content,
                sources,
            ));
        }
    }
    let mut scan = plans::scan(found, root.join(SPECIFY_DIR).is_dir());
    scan.behind = behind(&root);
    scan
}

/// How far the working copy's branch is behind its upstream, from what was fetched last; `None`
/// when it is not, has no upstream, or `git` cannot say. Nothing is fetched here.
pub(crate) fn behind(root: &Path) -> Option<WorkingCopyBehind> {
    let git = Git::new("git");
    let repo = root.to_string_lossy();
    let branch = git
        .run(&repo, &["symbolic-ref", "--short", "-q", "HEAD"])
        .ok()?;
    let upstream = git
        .run(&repo, &["rev-parse", "--abbrev-ref", "@{upstream}"])
        .ok()?;
    let count = git
        .run(&repo, &["rev-list", "--count", "HEAD..@{upstream}"])
        .ok()?;
    let behind: u32 = count.parse().ok().filter(|n| *n > 0)?;
    Some(WorkingCopyBehind {
        branch,
        upstream,
        behind,
    })
}

/// What is at a plan file: its text, or that it is missing or cannot be read.
fn content(file: &Path) -> Content {
    match std::fs::metadata(file) {
        Ok(m) if m.is_file() && m.len() <= MAX_BYTES => std::fs::read(file)
            .ok()
            .and_then(|b| String::from_utf8(b).ok())
            .map_or(Content::Unreadable, Content::Text),
        Ok(m) if m.is_file() => Content::Unreadable,
        _ => Content::Missing,
    }
}
