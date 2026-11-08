//! `tok review-mode`: who reviews finished cards, set on a project, a chain or one ticket.

use anyhow::{Result, bail};
use toknila_api_types::{ChainSettings, PatchProject, PatchTicket};
use toknila_client::Client;
use toknila_core::wire::from_wire_name;

use super::{default_project, emit};
use crate::output;

/// What the setting applies to.
#[derive(Debug)]
pub(super) enum Target {
    Ticket(String),
    Chain(String),
    /// A project key, or the only project.
    Project(Option<String>),
}

impl Target {
    /// The target the flags name; the project when none is given.
    pub(super) fn of(
        ticket: Option<String>,
        chain: Option<String>,
        project: Option<String>,
    ) -> Target {
        match (ticket, chain) {
            (Some(t), _) => Target::Ticket(t),
            (None, Some(c)) => Target::Chain(c),
            (None, None) => Target::Project(project),
        }
    }
}

pub(super) fn run(
    c: &Client,
    mode: Option<String>,
    target: Target,
    reviewer: Option<String>,
    json_out: bool,
) -> Result<()> {
    if mode.is_none() && reviewer.is_none() {
        bail!("give a mode (human, agent or both), a --reviewer profile, or both");
    }
    let review_mode = mode
        .as_deref()
        .map(|m| from_wire_name(m).ok_or_else(|| anyhow::anyhow!("unknown review mode {m}")))
        .transpose()?;
    match target {
        Target::Ticket(ticket) => {
            let patch = PatchTicket {
                review_mode,
                reviewer,
                ..PatchTicket::default()
            };
            emit(
                json_out,
                &c.patch_ticket(&ticket, &patch)?,
                output::ticket_line,
            );
        }
        Target::Chain(chain) => {
            let settings = ChainSettings {
                review_mode,
                reviewer,
                autostart: None,
            };
            emit(
                json_out,
                &c.patch_chain(&chain, None, &settings)?,
                output::chain,
            );
        }
        Target::Project(project) => {
            let key = default_project(c, project)?;
            let patch = PatchProject {
                review_mode,
                reviewer,
                ..PatchProject::default()
            };
            emit(json_out, &c.patch_project(&key, &patch)?, |p| {
                println!(
                    "{}  review mode: {}  reviewer: {}",
                    p.key,
                    output::w(&p.default_review_mode),
                    p.default_reviewer
                        .as_deref()
                        .unwrap_or("(<worker profile>-reviewer)")
                );
            });
        }
    }
    Ok(())
}
