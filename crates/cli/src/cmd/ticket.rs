//! `tok ticket`, `tok subtask`, `tok comment`, the ticket actions, `tok ls` and `tok show`.

use anyhow::{Context, Result};
use clap::Subcommand;
use toknila_api_types::{NewComment, NewTicket, PatchSubtask, TicketQuery};
use toknila_client::Client;
use toknila_core::wire::from_wire_name;

use super::emit;
use crate::output;

#[derive(Debug, Subcommand)]
pub enum TicketCmd {
    /// Add a ticket to a chain.
    New {
        chain: String,
        title: String,
        /// Dependencies (ticket keys, titles or release versions); repeatable.
        #[arg(long)]
        after: Vec<String>,
        #[arg(long)]
        profile: Option<String>,
        #[arg(long)]
        prompt: Option<String>,
        /// Acceptance criterion; repeatable.
        #[arg(long)]
        accept: Vec<String>,
        /// Checklist item; repeatable.
        #[arg(long)]
        subtask: Vec<String>,
        /// The agent shows a design for your approval before it writes any code.
        #[arg(long)]
        design_first: bool,
        /// The model of the card's agent (an alias such as opus, or a full name); left out, the
        /// project's worker model runs.
        #[arg(long)]
        model: Option<String>,
    },
    /// Show a ticket.
    Show { ticket: String },
}

#[derive(Debug, Subcommand)]
pub enum SubtaskCmd {
    /// Add a checklist item.
    Add { ticket: String, text: String },
    /// Tick a checklist item (1-based position).
    Done { ticket: String, position: u32 },
}

pub(super) fn run(c: &Client, cmd: TicketCmd, json_out: bool) -> Result<()> {
    match cmd {
        TicketCmd::New {
            chain,
            title,
            after,
            profile,
            prompt,
            accept,
            subtask,
            design_first,
            model,
        } => {
            let input = NewTicket {
                title,
                after,
                profile,
                prompt: prompt.unwrap_or_default(),
                acceptance: accept,
                subtasks: subtask,
                design_first,
                model,
                ..NewTicket::default()
            };
            emit(
                json_out,
                &c.create_ticket(&chain, &input)?,
                output::ticket_line,
            );
        }
        TicketCmd::Show { ticket } => emit(json_out, &c.ticket(&ticket)?, output::ticket),
    }
    Ok(())
}

pub(super) fn subtask(c: &Client, cmd: SubtaskCmd, json_out: bool) -> Result<()> {
    let detail = match cmd {
        SubtaskCmd::Add { ticket, text } => c.add_subtask(&ticket, &text)?,
        SubtaskCmd::Done { ticket, position } => {
            let done = PatchSubtask {
                done: Some(true),
                text: None,
            };
            c.patch_subtask(&ticket, position, &done)?
        }
    };
    emit(json_out, &detail, output::ticket);
    Ok(())
}

pub(super) fn comment(
    c: &Client,
    ticket: &str,
    text: String,
    kind: &str,
    json_out: bool,
) -> Result<()> {
    let input = NewComment {
        kind: Some(from_wire_name(kind).context("unknown comment kind")?),
        text: Some(text),
        body: None,
    };
    emit(
        json_out,
        &c.add_comment(ticket, &input)?,
        output::ticket_line,
    );
    Ok(())
}

/// A person's action on a ticket (`start`, `block`, ...).
pub(super) fn act(
    c: &Client,
    ticket: &str,
    action: &str,
    text: Option<&str>,
    json_out: bool,
) -> Result<()> {
    let detail = c.ticket_action(ticket, action, text)?;
    emit(json_out, &detail, output::ticket_line);
    Ok(())
}

pub(super) fn ls(
    c: &Client,
    needs_human: bool,
    chain: Option<String>,
    json_out: bool,
) -> Result<()> {
    let items = c.tickets(&TicketQuery { chain, needs_human })?;
    emit(json_out, &items, |items| output::tickets(items));
    Ok(())
}
