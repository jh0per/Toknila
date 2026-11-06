//! `tok profile`.

use std::path::Path;

use anyhow::Result;
use clap::Subcommand;
use toknila_client::Client;

use super::emit;
use crate::output;

#[derive(Debug, Subcommand)]
pub enum ProfileCmd {
    /// List the built-in profiles and the files of the profiles folder. Invalid files are named
    /// with their line on stderr.
    Ls,
}

pub(super) fn run(c: &Client, cmd: ProfileCmd, json_out: bool) -> Result<()> {
    match cmd {
        ProfileCmd::Ls => {
            let list = c.profiles()?;
            emit(json_out, &list, |l| {
                let rows: Vec<Vec<String>> = l
                    .profiles
                    .iter()
                    .map(|p| {
                        vec![
                            p.profile.name.clone(),
                            format!("{:?}", p.profile.role).to_lowercase(),
                            command(&p.profile.command),
                            p.file.clone().unwrap_or_else(|| "built-in".to_owned()),
                        ]
                    })
                    .collect();
                output::table(&["NAME", "ROLE", "COMMAND", "SOURCE"], &rows);
                for p in &l.problems {
                    let file = Path::new(&l.folder).join(&p.file);
                    eprintln!("{}:{}:{}: {}", file.display(), p.line, p.column, p.message);
                }
            });
        }
    }
    Ok(())
}

/// The argv for a table cell; the simulated profile starts nothing.
fn command(argv: &[String]) -> String {
    if argv.is_empty() {
        "(none: events come from `tok sim`)".to_owned()
    } else {
        argv.join(" ")
    }
}
