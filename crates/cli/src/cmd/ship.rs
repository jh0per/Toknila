//! `tok ship`: the ships of a project's cards.

use anyhow::Result;
use clap::Subcommand;
use toknila_client::Client;

use super::{default_project, emit};

/// `tok ship` subcommands.
#[derive(Debug, Subcommand)]
pub enum ShipCmd {
    /// Delete the leftover workspaces and `ship/*` branches that no live story owns: those of
    /// merged, cancelled and deleted stories. Same as Clean up in Settings, Leftover ships.
    Prune {
        /// A project (key); the only one when none is named.
        #[arg(long)]
        project: Option<String>,
        /// Only list what would go.
        #[arg(long)]
        dry_run: bool,
    },
}

/// Runs a `tok ship` command.
pub fn run(c: &Client, cmd: ShipCmd, json_out: bool) -> Result<()> {
    let ShipCmd::Prune { project, dry_run } = cmd;
    let project = default_project(c, project)?;
    if dry_run {
        let list = c.strays(&project)?;
        emit(json_out, &list, |l| {
            for s in &l.items {
                println!("{}", line(s));
            }
            println!("{} leftover(s)", l.items.len());
        });
        return Ok(());
    }
    let done = c.clean_strays(&project)?;
    emit(json_out, &done, |d| {
        for s in &d.removed {
            println!("removed {}", line(s));
        }
        for w in &d.warnings {
            println!("warning: {w}");
        }
        println!("{} leftover(s) removed", d.removed.len());
    });
    Ok(())
}

/// One leftover as a line: its name, the card it belonged to and the commit that merged it.
fn line(s: &toknila_core::strays::Stray) -> String {
    let mut out = s.name.clone();
    if let Some(card) = &s.card {
        out.push_str(&format!(" ({card}"));
        if let Some(c) = &s.merge_commit {
            out.push_str(&format!(", merged in {c}"));
        }
        out.push(')');
    }
    out
}
