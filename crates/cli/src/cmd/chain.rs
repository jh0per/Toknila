//! `tok chain` and `tok dep`.

use anyhow::{Context, Result, bail};
use clap::{Args, Subcommand};
use toknila_api_types::{EdgeInput, NewChain};
use toknila_client::Client;
use toknila_core::EdgeKind;

use super::{default_project, emit};
use crate::output::{self, w};

#[derive(Debug, Subcommand)]
pub enum ChainCmd {
    /// Create an empty chain.
    New {
        name: String,
        #[arg(long)]
        goal: Option<String>,
        /// Do not start Ready tickets automatically.
        #[arg(long)]
        no_autostart: bool,
        #[arg(long)]
        project: Option<String>,
    },
    /// Create a chain from a TOML file, or from a spec folder (with tasks.md); a folder of spec
    /// folders imports each one as its own chain.
    Import {
        file: std::path::PathBuf,
        #[arg(long)]
        project: Option<String>,
    },
    /// Write a chain as TOML (to stdout, or a file), or patch a spec folder's tasks.md with
    /// `--spec`: checkboxes follow the subtasks, new cards are appended, nothing is deleted.
    Export {
        chain: String,
        file: Option<std::path::PathBuf>,
        /// The spec folder whose tasks.md to patch.
        #[arg(long, conflicts_with = "file")]
        spec: Option<std::path::PathBuf>,
    },
    /// Show a chain as a graph in dependency order with its status.
    Show { chain: String },
    /// List chains with their status.
    Ls,
}

#[derive(Debug, Args)]
pub struct DepArgs {
    /// The dependency (ticket or release).
    from: String,
    /// The node that waits.
    to: String,
    /// Chain, when neither side is a ticket.
    #[arg(long)]
    chain: Option<String>,
}

#[derive(Debug, Subcommand)]
pub enum DepCmd {
    /// `to` waits for `from`.
    Add {
        #[command(flatten)]
        args: DepArgs,
        /// Wait until the release is released, not just cut.
        #[arg(long)]
        after_release: bool,
    },
    /// Remove a dependency.
    Rm {
        #[command(flatten)]
        args: DepArgs,
    },
}

pub(super) fn run(c: &Client, cmd: ChainCmd, json_out: bool) -> Result<()> {
    match cmd {
        ChainCmd::New {
            name,
            goal,
            no_autostart,
            project,
        } => {
            let p = default_project(c, project)?;
            let input = NewChain {
                name,
                goal,
                autostart: Some(!no_autostart),
                review_mode: None,
            };
            emit(json_out, &c.create_chain(&p, &input)?, output::chain);
        }
        ChainCmd::Import { file, project } if file.is_dir() => {
            return super::spec::import(c, &file, project, json_out);
        }
        ChainCmd::Import { file, project } => {
            let text = std::fs::read_to_string(&file)
                .with_context(|| format!("read {}", file.display()))?;
            let p = default_project(c, project)?;
            emit(json_out, &c.import_chain(&p, text)?, output::chain);
        }
        ChainCmd::Export {
            chain,
            spec: Some(dir),
            ..
        } => return super::spec::export(c, &chain, &dir),
        ChainCmd::Export { chain, file, .. } => {
            let toml = c.export_chain(&chain)?;
            match file {
                Some(f) => {
                    std::fs::write(&f, toml).with_context(|| format!("write {}", f.display()))?
                }
                None => print!("{toml}"),
            }
        }
        ChainCmd::Show { chain } => emit(json_out, &c.chain(&chain)?, output::chain),
        ChainCmd::Ls => {
            emit(json_out, &c.chains()?, |items| {
                let rows: Vec<Vec<String>> = items
                    .iter()
                    .map(|it| {
                        let s = &it.summary;
                        vec![
                            it.chain.name.clone(),
                            w(&s.status),
                            format!("{}/{}", s.done, s.total),
                            s.stopped_at
                                .as_ref()
                                .map(|x| x.ticket.to_string())
                                .unwrap_or_default(),
                        ]
                    })
                    .collect();
                output::table(&["CHAIN", "STATUS", "DONE", "STOPPED AT"], &rows);
            });
        }
    }
    Ok(())
}

pub(super) fn dep(c: &Client, cmd: DepCmd, json_out: bool) -> Result<()> {
    let detail = match cmd {
        DepCmd::Add {
            args,
            after_release,
        } => {
            let chain = chain_of(c, &[&args.to, &args.from], args.chain)?;
            let kind = if after_release {
                EdgeKind::AfterRelease
            } else {
                EdgeKind::Normal
            };
            let e = EdgeInput {
                from: args.from,
                to: args.to,
                kind,
            };
            c.add_edge(&chain, &e)?
        }
        DepCmd::Rm { args } => {
            let chain = chain_of(c, &[&args.to, &args.from], args.chain)?;
            c.remove_edge(&chain, &args.from, &args.to)?
        }
    };
    emit(json_out, &detail, output::chain);
    Ok(())
}

/// The chain of the first reference that is a ticket.
fn chain_of(c: &Client, refs: &[&str], given: Option<String>) -> Result<String> {
    if let Some(chain) = given {
        return Ok(chain);
    }
    for r in refs {
        if let Ok(t) = c.ticket(r) {
            return Ok(t.ticket.chain_id.to_string());
        }
    }
    bail!(
        "cannot tell the chain from {}; pass --chain",
        refs.join(" and ")
    )
}
