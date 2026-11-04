//! `tok project`.

use anyhow::Result;
use clap::Subcommand;
use toknila_api_types::NewProject;
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
        #[arg(long)]
        repo: Option<String>,
    },
    /// List projects.
    Ls,
}

pub(super) fn run(c: &Client, cmd: ProjectCmd, json_out: bool) -> Result<()> {
    match cmd {
        ProjectCmd::New { key, name, repo } => {
            let p = c.create_project(&NewProject {
                key: key.to_uppercase(),
                name: Some(name.unwrap_or(key)),
                repo_path: repo,
                ..NewProject::default()
            })?;
            emit(json_out, &p, |p| println!("project {} created", p.key));
        }
        ProjectCmd::Ls => {
            let projects = c.projects()?;
            emit(json_out, &projects, |ps| {
                let rows: Vec<Vec<String>> = ps
                    .iter()
                    .map(|p| vec![p.key.to_string(), p.name.clone()])
                    .collect();
                output::table(&["KEY", "NAME"], &rows);
            });
        }
    }
    Ok(())
}
