//! Every `tok` subcommand: parse, call the daemon through the typed client, print a table or the
//! JSON answer. One module per command group; no business rules here.

mod chain;
mod logs;
mod profile;
mod project;
mod release;
mod sim;
mod spec;
mod thread;
mod ticket;
mod watch;
mod wildcard;

use anyhow::{Context, Result, bail};
use clap::Subcommand;
use serde::Serialize;
use toknila_client::Client;
use toknila_daemon::paths::Paths;

pub use chain::{ChainCmd, DepCmd};
pub use profile::ProfileCmd;
pub use project::ProjectCmd;
pub use release::ReleaseCmd;
pub use sim::SimCmd;
pub use thread::ThreadCmd;
pub use ticket::{SubtaskCmd, TicketCmd};
pub use wildcard::CardArgs;

use crate::daemon_cmd;

/// Top-level commands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Start, stop or inspect the background daemon.
    #[command(subcommand)]
    Daemon(crate::daemon_cmd::DaemonCmd),
    /// Projects: one product, one repository.
    #[command(subcommand)]
    Project(ProjectCmd),
    /// Agent profiles: how each agent is launched.
    #[command(subcommand)]
    Profile(ProfileCmd),
    /// Chains of tickets.
    #[command(subcommand)]
    Chain(ChainCmd),
    /// Create or show tickets.
    #[command(subcommand)]
    Ticket(TicketCmd),
    /// Dependencies between tickets and milestones.
    #[command(subcommand)]
    Dep(DepCmd),
    /// Checklist items of a ticket.
    #[command(subcommand)]
    Subtask(SubtaskCmd),
    /// Threads on a card: settle a design, a flow or an architecture before implementation.
    #[command(subcommand)]
    Thread(ThreadCmd),
    /// Write to a ticket: an instruction for its agent, or a report or decision.
    Comment {
        ticket: String,
        text: String,
        #[arg(long, value_parser = ["instruction", "report", "decision"], default_value = "instruction")]
        kind: String,
    },
    /// Start a Ready ticket by hand.
    Start { ticket: String },
    /// Park a ticket in Backlog.
    Pause { ticket: String },
    /// Take a ticket off pause.
    Resume { ticket: String },
    /// Block a ticket, with a reason.
    Block { ticket: String, reason: String },
    /// Unblock a ticket; it returns to where it was.
    Unblock { ticket: String },
    /// Accept a ticket in Review: it becomes Merged (Done).
    Accept { ticket: String },
    /// Send a ticket in Review back to its agent with a comment.
    Return { ticket: String, comment: String },
    /// Reopen a Merged ticket for review.
    Reopen { ticket: String },
    /// Cancel a ticket (won't do), with a reason.
    Cancel { ticket: String, reason: String },
    /// Propose a wildcard card: work nobody planned. It waits for a person's approval.
    Propose {
        chain: String,
        #[command(flatten)]
        card: CardArgs,
        /// The ticket during which the work was found.
        #[arg(long)]
        from: Option<String>,
    },
    /// Approve a wildcard card: it joins the plan and its dependencies take effect.
    Approve { ticket: String },
    /// Reject a wildcard card, with a reason: it is cancelled.
    Reject { ticket: String, reason: String },
    /// Release milestones.
    #[command(subcommand)]
    Release(ReleaseCmd),
    /// Act as the simulated agent of a ticket or release (phase 0).
    Sim {
        /// Ticket key/title, or release version.
        target: String,
        #[command(subcommand)]
        action: SimCmd,
    },
    /// List tickets.
    Ls {
        /// Only tickets waiting on a person.
        #[arg(long)]
        needs_human: bool,
        #[arg(long)]
        chain: Option<String>,
    },
    /// Show one ticket with its subtasks and comments.
    Show { ticket: String },
    /// Print what a ticket's agent has written to its terminal; `--follow` keeps going live.
    Logs {
        ticket: String,
        /// Keep printing as the agent writes, until its run ends.
        #[arg(long, short)]
        follow: bool,
    },
    /// Print every change live; resumes after a reconnect.
    Watch {
        /// Start after this event seq (0 = from the beginning).
        #[arg(long)]
        since: Option<i64>,
    },
    /// Print the web page address of the daemon.
    Open,
}

/// Prints `v` as JSON (keys sorted, as before typed answers), or with the human formatter.
fn emit<T: Serialize>(json_out: bool, v: &T, human: impl FnOnce(&T)) {
    if json_out {
        let value = serde_json::to_value(v).unwrap_or_default();
        println!(
            "{}",
            serde_json::to_string_pretty(&value).unwrap_or_default()
        );
    } else {
        human(v);
    }
}

/// Project to use when none is given: the only one there is.
fn default_project(c: &Client, given: Option<String>) -> Result<String> {
    if let Some(p) = given {
        return Ok(p);
    }
    match c.projects()?.as_slice() {
        [one] => Ok(one.key.to_string()),
        [] => bail!("no project yet; create one with `tok project new <key>`"),
        _ => bail!("several projects exist; pass --project <key>"),
    }
}

/// Runs one command against the daemon of this home, starting it when needed.
pub fn run(cmd: Command, json_out: bool) -> Result<()> {
    if let Command::Daemon(d) = cmd {
        return daemon_cmd::run(d, json_out);
    }
    let paths = Paths::resolve().map_err(anyhow::Error::msg)?;
    let c = crate::client::connect(&paths)?;
    match cmd {
        Command::Daemon(_) => unreachable!("handled above"),
        Command::Project(p) => project::run(&c, p, json_out),
        Command::Profile(p) => profile::run(&c, p, json_out),
        Command::Chain(ch) => chain::run(&c, ch, json_out),
        Command::Dep(d) => chain::dep(&c, d, json_out),
        Command::Ticket(t) => ticket::run(&c, t, json_out),
        Command::Subtask(s) => ticket::subtask(&c, s, json_out),
        Command::Thread(t) => thread::run(&c, t, json_out),
        Command::Comment { ticket, text, kind } => {
            ticket::comment(&c, &ticket, text, &kind, json_out)
        }
        Command::Start { ticket } => ticket::act(&c, &ticket, "start", None, json_out),
        Command::Pause { ticket } => ticket::act(&c, &ticket, "pause", None, json_out),
        Command::Resume { ticket } => ticket::act(&c, &ticket, "resume", None, json_out),
        Command::Block { ticket, reason } => {
            ticket::act(&c, &ticket, "block", Some(&reason), json_out)
        }
        Command::Unblock { ticket } => ticket::act(&c, &ticket, "unblock", None, json_out),
        Command::Accept { ticket } => ticket::act(&c, &ticket, "accept", None, json_out),
        Command::Return { ticket, comment } => {
            ticket::act(&c, &ticket, "return", Some(&comment), json_out)
        }
        Command::Reopen { ticket } => ticket::act(&c, &ticket, "reopen", None, json_out),
        Command::Cancel { ticket, reason } => {
            ticket::act(&c, &ticket, "cancel", Some(&reason), json_out)
        }
        Command::Propose { chain, card, from } => {
            wildcard::propose(&c, &chain, card, from, json_out)
        }
        Command::Approve { ticket } => ticket::act(&c, &ticket, "approve", None, json_out),
        Command::Reject { ticket, reason } => {
            ticket::act(&c, &ticket, "reject", Some(&reason), json_out)
        }
        Command::Release(r) => release::run(&c, r, json_out),
        Command::Sim { target, action } => sim::run(&c, &target, action, json_out),
        Command::Ls { needs_human, chain } => ticket::ls(&c, needs_human, chain, json_out),
        Command::Show { ticket } => ticket::run(&c, TicketCmd::Show { ticket }, json_out),
        Command::Logs { ticket, follow } => logs::run(&c, &ticket, follow),
        Command::Watch { since } => watch::run(&c, &paths, since),
        Command::Open => {
            let info = daemon_cmd::running(&paths)?.context("daemon is not running")?;
            let url = format!("{}/?token={}", info.base_url(), info.token);
            emit(json_out, &serde_json::json!({ "url": url }), |_| {
                println!("{url}")
            });
            Ok(())
        }
    }
}
