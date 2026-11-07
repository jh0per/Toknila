//! The system Trash, behind a trait so tests move folders into a folder of their own. The
//! daemon never deletes a repository folder; at most it hands it to the Trash, after
//! `toknila_core::removal::may_trash` allowed it.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Names a folder that takes the place of the system Trash. Only the browser tests set it, so a
/// test deleting a project never reaches the person's Trash.
pub const TRASH_DIR_ENV: &str = "TOKNILA_TRASH_DIR";

/// Moves a folder to the Trash.
pub trait Trasher: Send + Sync + std::fmt::Debug {
    /// Moves the folder at `path` (absolute, existing) to the Trash, or says why not.
    fn trash(&self, path: &Path) -> Result<(), String>;
}

/// The Trash of this machine (the `trash` crate). On macOS it calls the system's file manager
/// API rather than scripting a desktop app, so a background daemon needs no extra permission.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemTrash;

#[cfg(target_os = "macos")]
fn context() -> trash::TrashContext {
    use trash::macos::{DeleteMethod, TrashContextExtMacos};
    let mut ctx = trash::TrashContext::default();
    ctx.set_delete_method(DeleteMethod::NsFileManager);
    ctx
}

#[cfg(not(target_os = "macos"))]
fn context() -> trash::TrashContext {
    trash::TrashContext::default()
}

impl Trasher for SystemTrash {
    fn trash(&self, path: &Path) -> Result<(), String> {
        context().delete(path).map_err(|e| e.to_string())
    }
}

/// A folder standing in for the Trash ([`TRASH_DIR_ENV`]): folders are moved into it, renamed
/// with a number when the name is taken.
#[derive(Debug, Clone)]
pub struct DirTrash(pub PathBuf);

impl Trasher for DirTrash {
    fn trash(&self, path: &Path) -> Result<(), String> {
        let name = path
            .file_name()
            .ok_or_else(|| format!("{} has no name", path.display()))?
            .to_string_lossy()
            .into_owned();
        std::fs::create_dir_all(&self.0).map_err(|e| e.to_string())?;
        let mut target = self.0.join(&name);
        for n in 1.. {
            if !target.exists() {
                break;
            }
            target = self.0.join(format!("{name}-{n}"));
        }
        std::fs::rename(path, &target).map_err(|e| {
            format!(
                "cannot move {} to {}: {e}",
                path.display(),
                target.display()
            )
        })
    }
}

/// The Trash a daemon uses: the folder [`TRASH_DIR_ENV`] names when it is set (`value`), else
/// the system Trash.
pub fn trasher_for(value: Option<OsString>) -> Arc<dyn Trasher> {
    match value.filter(|v| !v.is_empty()) {
        Some(dir) => Arc::new(DirTrash(PathBuf::from(dir))),
        None => Arc::new(SystemTrash),
    }
}

/// A daemon that may not move anything to the Trash; every request is refused.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoTrash;

impl Trasher for NoTrash {
    fn trash(&self, path: &Path) -> Result<(), String> {
        Err(format!(
            "this daemon has no Trash; {} stays",
            path.display()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_trash_folder_variable_moves_folders_into_that_folder_never_the_system_trash() {
        let dir = tempfile::tempdir().expect("temp dir");
        let bin = dir.path().join("bin");
        let trasher = trasher_for(Some(bin.clone().into_os_string()));
        assert!(
            format!("{trasher:?}").starts_with("DirTrash"),
            "{trasher:?}"
        );
        for _ in 0..2 {
            let repo = dir.path().join("repo");
            std::fs::create_dir_all(repo.join("src")).expect("repo");
            trasher.trash(&repo).expect("moved");
            assert!(!repo.exists());
        }
        assert!(bin.join("repo/src").is_dir());
        assert!(
            bin.join("repo-1/src").is_dir(),
            "a taken name gets a number"
        );
    }

    #[test]
    fn without_the_variable_the_system_trash_is_used() {
        assert_eq!(format!("{:?}", trasher_for(None)), "SystemTrash");
        assert_eq!(
            format!("{:?}", trasher_for(Some(OsString::new()))),
            "SystemTrash"
        );
    }
}
