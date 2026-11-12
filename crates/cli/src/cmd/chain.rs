//! `tok chain` and `tok dep`.

use anyhow::{Context, Result, bail};
use clap::{Args, Subcommand};
use toknila_api_types::{ChainSettings, EdgeInput, EdgeMoveInput, MoveChain, NewChain};
use toknila_client::Client;
use toknila_core::EdgeKind;

use super::{default_project, emit};
use crate::output::{self, w};

#[derive(Debug, Subcommand)]
pub enum ChainCmd {
    /// Create an empty epic.
    New {
        name: String,
        #[arg(long)]
        goal: Option<String>,
        /// The version the epic releases, such as `0.3.0`: its end release is made with it.
        #[arg(long)]
        version: Option<String>,
        /// Do not start stories automatically (a person starts each one).
        #[arg(long)]
        no_autostart: bool,
        #[arg(long)]
        project: Option<String>,
    },
    /// Create an epic from a TOML file, or from a spec folder (with tasks.md); a folder of spec
    /// folders imports each one as its own epic.
    Import {
        file: std::path::PathBuf,
        #[arg(long)]
        project: Option<String>,
    },
    /// Write an epic as TOML (to stdout, or a file), or patch a spec folder's tasks.md with
    /// `--spec`: checkboxes follow the tasks, new stories are appended, nothing is deleted.
    Export {
        chain: String,
        file: Option<std::path::PathBuf>,
        /// The spec folder whose tasks.md to patch.
        #[arg(long, conflicts_with = "file")]
        spec: Option<std::path::PathBuf>,
    },
    /// Change an epic's settings after it is made. `--autostart on` starts stories automatically:
    /// the ready ones now (as far as slots allow) and later ones as they become ready; `off` stops
    /// that and opens the epic's graph to the planner.
    Set {
        chain: String,
        /// The project, when two projects have an epic of this name.
        #[arg(long)]
        project: Option<String>,
        /// Whether the epic starts its stories automatically.
        #[arg(long, value_parser = ["on", "off"])]
        autostart: Option<String>,
        /// The version the epic releases; refused once its release was cut.
        #[arg(long)]
        version: Option<String>,
    },
    /// Move an epic in its project's roadmap: right before or right after another epic.
    Mv {
        chain: String,
        /// Put it right before this epic.
        #[arg(long, conflicts_with = "after", required_unless_present = "after")]
        before: Option<String>,
        /// Put it right after this epic.
        #[arg(long)]
        after: Option<String>,
        /// The project, when two projects have an epic of this name.
        #[arg(long)]
        project: Option<String>,
    },
    /// Show an epic as a graph in dependency order with its status.
    Show { chain: String },
    /// List epics with their status.
    Ls,
    /// Delete an epic with its stories, threads and workspaces (the project folder and the branches
    /// made for the stories stay). Refused while an agent works on one of its stories.
    Rm {
        chain: String,
        /// The project, when two projects have an epic of this name.
        #[arg(long)]
        project: Option<String>,
        /// Do not ask for confirmation.
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Debug, Args)]
pub struct DepArgs {
    /// The dependency (ticket or release).
    from: String,
    /// The node that waits.
    to: String,
    /// Epic, when neither side is a ticket.
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
    /// Move a dependency to another end in one step: `from`/`to` name it, `--new-from` and
    /// `--new-to` say what changes. A refusal (a cycle, a story that started) changes nothing.
    Mv {
        #[command(flatten)]
        args: DepArgs,
        /// The new dependency (defaults to `from`).
        #[arg(long)]
        new_from: Option<String>,
        /// The new node that waits (defaults to `to`).
        #[arg(long)]
        new_to: Option<String>,
        /// Wait until the release is released, not just cut.
        #[arg(long)]
        after_release: bool,
    },
}

pub(super) fn run(c: &Client, cmd: ChainCmd, json_out: bool) -> Result<()> {
    match cmd {
        ChainCmd::New {
            name,
            goal,
            version,
            no_autostart,
            project,
        } => {
            let p = default_project(c, project)?;
            let input = NewChain {
                name,
                goal,
                autostart: Some(!no_autostart),
                review_mode: None,
                reviewer: None,
                version,
            };
            emit(json_out, &c.create_chain(&p, &input)?, output::chain);
        }
        ChainCmd::Import { file, project } if file.is_dir() => {
            return super::spec::import(c, &file, project, json_out);
        }
        ChainCmd::Import { file, project } => {
            let detail = super::spec::import_file(c, &file, project)?;
            emit(json_out, &detail, output::chain);
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
        ChainCmd::Rm {
            chain,
            project,
            yes,
        } => return super::chain_rm::run(c, &chain, project.as_deref(), yes, json_out),
        ChainCmd::Set {
            chain,
            project,
            autostart,
            version,
        } => {
            if autostart.is_none() && version.is_none() {
                bail!("nothing to set: pass --autostart on|off or --version");
            }
            let settings = ChainSettings {
                autostart: autostart.map(|a| a == "on"),
                version,
                ..ChainSettings::default()
            };
            let detail = c.patch_chain(&chain, project.as_deref(), &settings)?;
            emit(json_out, &detail, output::chain);
        }
        ChainCmd::Mv {
            chain,
            before,
            after,
            project,
        } => {
            let to = MoveChain { before, after };
            let detail = c.move_chain(&chain, project.as_deref(), &to)?;
            emit(json_out, &detail, output::chain);
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
                            s.phase.word().to_owned(),
                            w(&s.status),
                            format!("{}/{}", s.done, s.total),
                            s.stopped_at
                                .as_ref()
                                .map(|x| x.ticket.to_string())
                                .unwrap_or_default(),
                        ]
                    })
                    .collect();
                output::table(&["CHAIN", "PHASE", "STATUS", "DONE", "STOPPED AT"], &rows);
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
        DepCmd::Mv {
            args,
            new_from,
            new_to,
            after_release,
        } => {
            if new_from.is_none() && new_to.is_none() {
                bail!("say where it goes: --new-from and/or --new-to");
            }
            let chain = chain_of(c, &[&args.to, &args.from], args.chain)?;
            let kind = if after_release {
                EdgeKind::AfterRelease
            } else {
                EdgeKind::Normal
            };
            let m = EdgeMoveInput {
                new: EdgeInput {
                    from: new_from.unwrap_or_else(|| args.from.clone()),
                    to: new_to.unwrap_or_else(|| args.to.clone()),
                    kind,
                },
                old: EdgeInput {
                    from: args.from,
                    to: args.to,
                    kind: EdgeKind::Normal,
                },
            };
            c.move_edge(&chain, &m)?
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
        "cannot tell the epic from {}; pass --chain",
        refs.join(" and ")
    )
}
