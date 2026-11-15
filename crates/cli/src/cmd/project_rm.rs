//! `tok project rm`: the confirmation before the folder goes to the Trash, and what a deletion
//! says afterwards.

use std::io::BufRead;

use anyhow::{Result, bail};
use toknila_api_types::ProjectDeletion;
use toknila_core::removal::FolderFate;

/// Asks the person to type the project's key before its folder goes to the Trash. Refuses when
/// nobody can answer (stdin is not a terminal) and when the answer is not the key.
pub(super) fn confirm(
    input: &mut impl BufRead,
    terminal: bool,
    key: &str,
    folder: &str,
) -> Result<()> {
    if !terminal {
        bail!(
            "not deleting {key}: moving {folder} to the Trash needs a confirmation and stdin is not a terminal; pass --yes"
        );
    }
    eprintln!("This deletes project {key} and moves {folder} to the Trash.");
    eprint!("Type the project key ({key}) to go on: ");
    let mut line = String::new();
    input.read_line(&mut line)?;
    if line.trim() != key {
        bail!("{:?} is not {key}; nothing was deleted", line.trim());
    }
    Ok(())
}

/// What a deletion did, for a person: what went, where the folder is, and what stayed behind.
pub(super) fn deleted(d: &ProjectDeletion) {
    println!(
        "project {} deleted: {} {} {:?}, {} {}",
        d.project.key,
        d.chains.len(),
        if d.chains.len() == 1 { "epic" } else { "epics" },
        d.chains,
        d.tickets,
        if d.tickets == 1 { "story" } else { "stories" }
    );
    for w in &d.worktrees_removed {
        println!("removed workspace {w}");
    }
    let folder = d.project.repo_path.as_deref().unwrap_or_default();
    match d.folder {
        FolderFate::Kept => println!("folder kept: {folder}"),
        FolderFate::Trashed => println!("folder moved to the Trash: {folder}"),
        FolderFate::None => println!("no folder"),
    }
    if let Some(e) = &d.folder_error {
        println!("the folder stayed where it was: {e}");
    }
    for w in &d.warnings {
        println!("warning: {w}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_trash_needs_the_key_typed_at_a_terminal() {
        let typed = |text: &str, terminal: bool| {
            confirm(&mut text.as_bytes(), terminal, "DEMO", "/w/demo").map_err(|e| e.to_string())
        };
        assert_eq!(typed("DEMO\n", true), Ok(()));
        assert_eq!(typed("  DEMO  \n", true), Ok(()));
        assert_eq!(
            typed("demo\n", true),
            Err("\"demo\" is not DEMO; nothing was deleted".to_owned())
        );
        assert_eq!(
            typed("", true),
            Err("\"\" is not DEMO; nothing was deleted".to_owned())
        );
        let e = typed("DEMO\n", false).expect_err("no terminal");
        assert!(e.contains("stdin is not a terminal; pass --yes"), "{e}");
    }
}
