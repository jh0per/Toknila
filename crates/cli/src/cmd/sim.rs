//! `tok sim`: act as the simulated agent of a ticket or a release.

use anyhow::{Context, Result};
use clap::Subcommand;
use toknila_api_types::{AgentEvent, ReleaseEvent};
use toknila_client::Client;
use toknila_core::{CommentBody, ItemOutcome, ItemVerdict, Verdict};

use super::thread::{PostArgs, kind_of};
use super::{CardArgs, emit};
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
    /// The simulated reviewer gives its verdict on a card in Review.
    Review {
        #[arg(value_parser = ["approve", "request-changes"])]
        verdict: String,
        /// What to change; required with request-changes unless an item fails.
        comment: Option<String>,
        /// The judgement of an acceptance item, `N=pass` or `N=fail` (repeat for each item).
        #[arg(long = "item")]
        items: Vec<String>,
    },
    /// The agent found work nobody planned: a wildcard card that waits for your approval.
    Propose {
        #[command(flatten)]
        card: CardArgs,
    },
    /// The agent works in threads on its card.
    #[command(subcommand)]
    Thread(SimThreadCmd),
}

/// What the simulated agent does in threads. It cannot approve: that is a person's.
#[derive(Debug, Subcommand)]
pub enum SimThreadCmd {
    /// Open a thread; the hand-in waits until it is approved.
    Open {
        #[arg(value_parser = ["design", "flow", "architecture", "other"])]
        kind: String,
        title: String,
    },
    /// Post a message and/or the next artifact version.
    Post {
        #[command(flatten)]
        post: PostArgs,
    },
}

/// `N=pass` or `N=fail` as the judgement of acceptance item N.
fn item_verdict(text: &str) -> Result<ItemVerdict> {
    let (n, outcome) = text
        .split_once('=')
        .with_context(|| format!("{text:?}: give an item as N=pass or N=fail"))?;
    Ok(ItemVerdict {
        item: n
            .trim()
            .parse()
            .with_context(|| format!("{n:?} is not an item number"))?,
        outcome: match outcome.trim() {
            "pass" => ItemOutcome::Pass,
            "fail" => ItemOutcome::Fail,
            other => anyhow::bail!("{other:?}: an item is judged pass or fail"),
        },
        basis: toknila_core::ItemBasis::default(),
        note: String::new(),
    })
}

pub(super) fn run(c: &Client, target: &str, action: SimCmd, json_out: bool) -> Result<()> {
    if let SimCmd::Thread(t) = action {
        let events = match t {
            SimThreadCmd::Open { kind, title } => vec![AgentEvent::OpenThread {
                kind: kind_of(&kind)?,
                title,
            }],
            SimThreadCmd::Post { post } => {
                let p = post.read()?;
                let thread = post.thread;
                let note = p.message.or_else(|| p.version.as_ref()?.message.clone());
                let version = p.version.map(|v| AgentEvent::ThreadVersion {
                    thread: thread.clone(),
                    format: v.format,
                    body: v.body,
                });
                let message = note.map(|text| AgentEvent::ThreadMessage { thread, text });
                version.into_iter().chain(message).collect()
            }
        };
        // A version and a message are two events; the answer is the ticket after the last.
        let mut last = None;
        for event in events {
            last = Some(c.agent_event(target, &event)?);
        }
        if let Some(detail) = last {
            emit(json_out, &detail, output::ticket_line);
        }
        return Ok(());
    }
    let event = match action {
        SimCmd::Start => AgentEvent::Started,
        SimCmd::Ask { text, permission } => AgentEvent::AskHuman {
            text,
            permission,
            detail: None,
        },
        SimCmd::Subtask { position, .. } => AgentEvent::SubtaskDone { position },
        SimCmd::Note { text } => AgentEvent::Note {
            body: CommentBody {
                gist: text,
                ..CommentBody::default()
            },
        },
        SimCmd::Crash { detail } => AgentEvent::Crashed { detail },
        SimCmd::Review {
            verdict,
            comment,
            items,
        } => AgentEvent::ReviewVerdict {
            verdict: if verdict == "approve" {
                Verdict::Approve
            } else {
                Verdict::RequestChanges
            },
            comment: comment.unwrap_or_default(),
            items: items
                .iter()
                .map(|i| item_verdict(i))
                .collect::<Result<_>>()?,
        },
        SimCmd::Propose { card } => AgentEvent::Propose(card.into()),
        SimCmd::Thread(_) => unreachable!("handled above"),
        // A ticket first; if no ticket has this name, a release.
        SimCmd::Finish { summary } => match c.ticket(target) {
            Ok(_) => AgentEvent::Finished {
                // A person at the terminal types plain words; the hand-in shape is the agent's.
                summary: toknila_core::handin::from_plain(
                    &summary.context("finish needs a summary for a ticket")?,
                ),
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
