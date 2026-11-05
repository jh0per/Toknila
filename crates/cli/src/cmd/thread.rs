//! `tok thread`: threads on a card, settled with the owner before implementation. A person opens
//! one, posts, approves or asks for changes; `tok sim <ticket> thread ...` is the agent's way in.

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::{Args, Subcommand};
use toknila_api_types::{NewMessage, NewThread, NewVersion, ThreadDetail, TicketThreads};
use toknila_client::Client;
use toknila_core::wire::from_wire_name;
use toknila_core::{ArtifactFormat, ThreadKind};

use super::emit;
use crate::output;

const KINDS: [&str; 4] = ["design", "flow", "architecture", "other"];
const FORMATS: [&str; 3] = ["mermaid", "markdown", "html"];

/// What goes into a thread with `post`: a message, an artifact version, or both.
#[derive(Debug, Args)]
pub struct PostArgs {
    /// Thread number on the ticket (1, 2, ...) or its id.
    pub thread: String,
    /// A message.
    #[arg(long)]
    pub message: Option<String>,
    /// A file holding the next artifact version.
    #[arg(long, requires = "format")]
    pub artifact: Option<PathBuf>,
    #[arg(long, value_parser = FORMATS, requires = "artifact")]
    pub format: Option<String>,
}

/// The two things a post can hold, checked and read.
#[derive(Debug)]
pub struct Post {
    pub message: Option<String>,
    pub version: Option<NewVersion>,
}

impl PostArgs {
    /// Reads the artifact file; a post needs a message or an artifact.
    pub fn read(&self) -> Result<Post> {
        let version = match (&self.artifact, &self.format) {
            (Some(path), Some(format)) => Some(NewVersion {
                format: format_of(format)?,
                body: std::fs::read_to_string(path)
                    .with_context(|| format!("cannot read {}", path.display()))?,
            }),
            _ => None,
        };
        if self.message.is_none() && version.is_none() {
            bail!("give --message, --artifact with --format, or both");
        }
        Ok(Post {
            message: self.message.clone(),
            version,
        })
    }
}

/// `tok thread ...`.
#[derive(Debug, Subcommand)]
pub enum ThreadCmd {
    /// List a ticket's threads and its phase (design while any is open).
    Ls { ticket: String },
    /// Open a thread: a design, a flow, an architecture or something else to settle first.
    Open {
        ticket: String,
        #[arg(value_parser = KINDS)]
        kind: String,
        title: String,
    },
    /// Post a message and/or the next artifact version; a new version reopens an approved thread.
    Post {
        ticket: String,
        #[command(flatten)]
        post: PostArgs,
    },
    /// Approve a thread; its latest version is pinned. Implementation waits for every thread.
    Approve { ticket: String, thread: String },
    /// Ask for changes: a message that keeps the thread open, or reopens it.
    Changes {
        ticket: String,
        thread: String,
        comment: String,
    },
}

pub(super) fn run(c: &Client, cmd: ThreadCmd, json_out: bool) -> Result<()> {
    match cmd {
        ThreadCmd::Ls { ticket } => emit(json_out, &c.threads(&ticket)?, list),
        ThreadCmd::Open {
            ticket,
            kind,
            title,
        } => {
            let input = NewThread {
                kind: kind_of(&kind)?,
                title,
            };
            emit(json_out, &c.open_thread(&ticket, &input)?, line);
        }
        ThreadCmd::Post { ticket, post } => {
            let p = post.read()?;
            let mut last = None;
            if let Some(v) = &p.version {
                last = Some(c.post_thread_version(&ticket, &post.thread, v)?);
            }
            if let Some(text) = p.message {
                let m = NewMessage { text };
                last = Some(c.post_thread_message(&ticket, &post.thread, &m)?);
            }
            if let Some(detail) = last {
                emit(json_out, &detail, line);
            }
        }
        ThreadCmd::Approve { ticket, thread } => {
            emit(json_out, &c.approve_thread(&ticket, &thread)?, line);
        }
        ThreadCmd::Changes {
            ticket,
            thread,
            comment,
        } => {
            let m = NewMessage { text: comment };
            emit(
                json_out,
                &c.request_thread_changes(&ticket, &thread, &m)?,
                line,
            );
        }
    }
    Ok(())
}

/// The thread kind named on the command line.
pub fn kind_of(s: &str) -> Result<ThreadKind> {
    from_wire_name(s).with_context(|| format!("unknown thread kind {s}"))
}

/// The artifact format named on the command line.
pub fn format_of(s: &str) -> Result<ArtifactFormat> {
    from_wire_name(s).with_context(|| format!("unknown artifact format {s}"))
}

/// `#1 open design "List screen" v2`.
pub fn line(d: &ThreadDetail) {
    let t = &d.thread;
    let pinned = t
        .approved_version
        .map(|v| format!(" pinned v{v}"))
        .unwrap_or_default();
    println!(
        "#{} {} {} \"{}\" v{}{pinned}",
        t.number,
        output::w(&t.state),
        output::w(&t.kind),
        t.title,
        d.versions.len()
    );
}

fn list(d: &TicketThreads) {
    println!("{} {}", d.ticket, output::w(&d.phase));
    let rows: Vec<Vec<String>> = d
        .threads
        .iter()
        .map(|t| {
            vec![
                format!("#{}", t.thread.number),
                output::w(&t.thread.state),
                output::w(&t.thread.kind),
                format!("v{}", t.versions.len()),
                t.messages.len().to_string(),
                t.thread.title.clone(),
            ]
        })
        .collect();
    output::table(
        &["THREAD", "STATE", "KIND", "VERSION", "MESSAGES", "TITLE"],
        &rows,
    );
}
