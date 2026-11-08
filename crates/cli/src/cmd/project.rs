//! `tok project`.

use std::io::{BufRead, IsTerminal};

use anyhow::{Result, anyhow, bail};
use clap::Subcommand;
use toknila_api_types::{DeleteProject, NewProject, PatchProject, ProjectDeletion};
use toknila_client::Client;
use toknila_core::removal::FolderFate;
use toknila_core::wire::from_wire_name;

use super::emit;
use crate::output;

#[derive(Debug, Subcommand)]
pub enum ProjectCmd {
    /// Create a project; the key prefixes ticket keys (demo -> DEMO-1).
    New {
        key: String,
        #[arg(long)]
        name: Option<String>,
        /// Path of the project's git repository; tickets get a worktree in it.
        #[arg(long)]
        repo: Option<String>,
        /// Branch ships start from (default: the branch the repository is on, else the remote's
        /// default branch).
        #[arg(long)]
        base: Option<String>,
        /// Agent profile tickets run with unless they name one (default: sim).
        #[arg(long)]
        profile: Option<String>,
    },
    /// Change a project's settings.
    Set {
        key: String,
        /// Most agents working at once (1 to 64, default 2); more started cards wait in a queue.
        #[arg(long)]
        parallel: Option<u32>,
        /// Where accepted work lands: `local` (merge into the base branch) or `github` (open a
        /// pull request, merge it there when its checks pass).
        #[arg(long)]
        merge: Option<String>,
        /// Pull request title; {key} and {title} are the card's (default: "{key}: {title}").
        #[arg(long)]
        pr_title: Option<String>,
        /// How a pull request merges: merge, squash or rebase (default: merge).
        #[arg(long)]
        merge_method: Option<String>,
        /// The command that checks the work (e.g. `cargo xtask verify`); agents are told to run it
        /// and may without asking. An empty text clears it.
        #[arg(long)]
        check_command: Option<String>,
    },
    /// List projects.
    Ls,
    /// Delete a project with its chains, cards and everything attached to them. The ships'
    /// worktrees and run files go too; the repository folder and its `ship/*` branches stay.
    Rm {
        key: String,
        /// Also move the repository folder to the system Trash; asks you to type the project key.
        #[arg(long)]
        trash_folder: bool,
        /// Do not ask (needed with --trash-folder when stdin is not a terminal).
        #[arg(long)]
        yes: bool,
    },
}

pub(super) fn run(c: &Client, cmd: ProjectCmd, json_out: bool) -> Result<()> {
    match cmd {
        ProjectCmd::New {
            key,
            name,
            repo,
            base,
            profile,
        } => {
            let p = c.create_project(&NewProject {
                key: key.to_uppercase(),
                name: Some(name.unwrap_or(key)),
                repo_path: repo,
                work_branch: base,
                default_profile: profile,
                ..NewProject::default()
            })?;
            emit(json_out, &p, |p| println!("project {} created", p.key));
        }
        ProjectCmd::Set {
            key,
            parallel,
            merge,
            pr_title,
            merge_method,
            check_command,
        } => {
            let patch = PatchProject {
                max_parallel: parallel,
                merge_mode: merge
                    .as_deref()
                    .map(|m| from_wire_name(m).ok_or_else(|| anyhow!("unknown merge mode {m}")))
                    .transpose()?,
                pr_title,
                merge_method: merge_method
                    .as_deref()
                    .map(|m| from_wire_name(m).ok_or_else(|| anyhow!("unknown merge method {m}")))
                    .transpose()?,
                check_command,
                ..PatchProject::default()
            };
            let p = c.patch_project(&key, &patch)?;
            emit(json_out, &p, |p| {
                println!(
                    "project {}: up to {} agents at once; merge: {} ({}), PR title {:?}",
                    p.key,
                    p.max_parallel,
                    output::w(&p.merge.mode),
                    output::w(&p.merge.method),
                    p.merge.pr_title
                );
                if let Some(c) = &p.check_command {
                    println!("check command: {c}");
                }
            });
        }
        ProjectCmd::Ls => {
            let projects = c.projects()?;
            emit(json_out, &projects, |ps| {
                let rows: Vec<Vec<String>> = ps
                    .iter()
                    .map(|p| {
                        vec![
                            p.key.to_string(),
                            p.name.clone(),
                            p.max_parallel.to_string(),
                        ]
                    })
                    .collect();
                output::table(&["KEY", "NAME", "AGENTS"], &rows);
            });
        }
        ProjectCmd::Rm {
            key,
            trash_folder,
            yes,
        } => {
            if trash_folder && !yes {
                let p = c.project(&key)?;
                let folder = p.repo_path.as_deref().unwrap_or("its folder (it has none)");
                let stdin = std::io::stdin();
                let terminal = stdin.is_terminal();
                confirm(&mut stdin.lock(), terminal, &p.key.to_string(), folder)?;
            }
            let out = c.delete_project(&key, &DeleteProject { trash_folder })?;
            emit(json_out, &out, deleted);
        }
    }
    Ok(())
}

/// Asks the person to type the project's key before its folder goes to the Trash. Refuses when
/// nobody can answer (stdin is not a terminal) and when the answer is not the key.
fn confirm(input: &mut impl BufRead, terminal: bool, key: &str, folder: &str) -> Result<()> {
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
fn deleted(d: &ProjectDeletion) {
    println!(
        "project {} deleted: {} chain(s) {:?}, {} card(s)",
        d.project.key,
        d.chains.len(),
        d.chains,
        d.tickets
    );
    for w in &d.worktrees_removed {
        println!("removed worktree {w}");
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
