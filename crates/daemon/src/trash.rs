//! The system Trash, behind a trait so tests move folders into a folder of their own. The
//! daemon never deletes a repository folder; at most it hands it to the Trash, after
//! `toknila_core::removal::may_trash` allowed it.

use std::path::Path;

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
