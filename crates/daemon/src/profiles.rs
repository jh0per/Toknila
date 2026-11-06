//! Reads the profiles folder: the adapter side of agent profiles. Which files are profiles and
//! whether they are valid is decided in `toknila_core::profile`.

use std::path::Path;

use toknila_core::profile::{Catalog, ProfileFile, catalog};

/// The files of `dir`, unread ones with the reason. A folder that does not exist (nobody wrote a
/// profile yet) has no files; one that cannot be listed has none either, and is logged.
fn read_folder(dir: &Path) -> Vec<ProfileFile> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) => {
            if e.kind() != std::io::ErrorKind::NotFound {
                tracing::warn!(dir = %dir.display(), "cannot list the profiles folder: {e}");
            }
            return Vec::new();
        }
    };
    entries
        .filter_map(Result::ok)
        .filter(|e| e.path().is_file())
        .filter_map(|e| {
            let file = e.file_name().into_string().ok()?;
            let text = std::fs::read_to_string(e.path()).map_err(|err| err.to_string());
            Some(ProfileFile { file, text })
        })
        .collect()
}

/// The built-in profiles and the valid files of `dir`, with the problems of the others.
pub fn load(dir: &Path) -> Catalog {
    catalog(read_folder(dir))
}
