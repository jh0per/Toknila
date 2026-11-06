//! `tok project`.

use anyhow::Result;
use clap::Subcommand;
use toknila_api_types::{NewProject, PatchProject};
use toknila_client::Client;

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
        /// Branch ships start from (default: develop).
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
    },
    /// List projects.
    Ls,
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
        ProjectCmd::Set { key, parallel } => {
            let patch = PatchProject {
                max_parallel: parallel,
                ..PatchProject::default()
            };
            let p = c.patch_project(&key, &patch)?;
            emit(json_out, &p, |p| {
                println!("project {}: up to {} agents at once", p.key, p.max_parallel);
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
    }
    Ok(())
}
