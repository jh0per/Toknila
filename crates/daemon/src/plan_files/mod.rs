//! Plan files on disk: reading what a person points the daemon at (a chain file, a spec folder,
//! a folder of spec folders) for an import, and finding the plans of a project's repository.
//! Both are blocking filesystem work, run off the store's thread and outside any transaction;
//! what is a plan and how a `tasks.md` becomes a chain are core's (`plans`, `spec`).

mod read;
mod scan;

use std::path::{Path, PathBuf};

pub use read::{Candidate, Found, Prepared, read};
pub use scan::scan;

/// `path` with links and `..` resolved, so the same place always reads the same; as given when
/// it cannot be resolved. A Windows verbatim prefix (`\\?\C:\`) is dropped for a drive path.
pub(crate) fn canonical(path: &Path) -> PathBuf {
    let Ok(resolved) = std::fs::canonicalize(path) else {
        return path.to_owned();
    };
    let text = resolved.to_string_lossy();
    match text.strip_prefix(r"\\?\") {
        Some(rest) if rest.as_bytes().get(1) == Some(&b':') => PathBuf::from(rest),
        _ => resolved,
    }
}

/// A path as text for the wire.
pub(crate) fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// The last part of a path: a folder's or a file's name.
fn name_of(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests;
