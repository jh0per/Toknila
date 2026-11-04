//! `tok sim`: act as the simulated agent of a ticket or a release (phase 0).

use anyhow::{Context, Result};
use clap::Subcommand;
use toknila_api_types::{AgentEvent, ReleaseEvent};
use toknila_client::Client;
use toknila_core::CommentBody;

use super::emit;
use crate::output;

#[derive(Debug, Subcommand)]
pub enum SimCmd {
    /// The agent started (Ready -> Running).
    Start,
    /// The agent asks a person something (-> Waiting).
    Ask {
        text: String,
        /// It is a permission request, not a question.
        #[arg(long)]
        permission: bool,
    },
    /// The agent ticks subtask N: `tok sim A subtask 1 done`.
    Subtask {
        position: u32,
        #[arg(value_parser = ["done"], default_value = "done")]
        state: String,
    },
    /// The agent reports progress.
    Note { text: String },
    /// The agent is done (-> Review); on a release: it is released.
    Finish { summary: Option<String> },
    /// The agent crashed (-> Blocked).
    Crash { detail: String },
}

pub(super) fn run(c: &Client, target: &str, action: SimCmd, json_out: bool) -> Result<()> {
    let event = match action {
        SimCmd::Start => AgentEvent::Started,
        SimCmd::Ask { text, permission } => AgentEvent::AskHuman { text, permission },
        SimCmd::Subtask { position, .. } => AgentEvent::SubtaskDone { position },
        SimCmd::Note { text } => AgentEvent::Note {
            body: CommentBody {
                gist: text,
                ..CommentBody::default()
            },
        },
        SimCmd::Crash { detail } => AgentEvent::Crashed { detail },
        // A ticket first; if no ticket has this name, a release.
        SimCmd::Finish { summary } => match c.ticket(target) {
            Ok(_) => AgentEvent::Finished {
                summary: summary.context("finish needs a summary for a ticket")?,
            },
            Err(_) => {
                let detail = c.release_event(target, &ReleaseEvent::Finished)?;
                emit(json_out, &detail, output::chain);
                return Ok(());
            }
        },
    };
    emit(
        json_out,
        &c.agent_event(target, &event)?,
        output::ticket_line,
    );
    Ok(())
}
