//! Every `tok` subcommand: parse, call the daemon through the typed client, print a table or the
//! JSON answer. One module per command group; no business rules here.

mod agent;
mod chain;
mod hook;
mod logs;
mod mcp;
mod mcp_tools;
mod profile;
mod project;
mod release;
mod review;
mod sim;
mod spec;
mod statusline;
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
    /// Accept a ticket in Review: it becomes Merged (Done). Remarks are optional.
    Accept {
        ticket: String,
        comment: Option<String>,
    },
    /// Choose who reviews finished cards: a person, a reviewer agent, or the agent first and
    /// then a person. Set on a project, a chain or one ticket; the ticket's wins, then the
    /// chain's, then the project's.
    ReviewMode {
        /// human, agent or both.
        #[arg(value_parser = ["human", "agent", "both"])]
        mode: Option<String>,
        /// A ticket (key or title).
        #[arg(long, conflicts_with_all = ["chain", "project"])]
        ticket: Option<String>,
        /// A chain (name).
        #[arg(long, conflicts_with = "project")]
        chain: Option<String>,
        /// A project (key); the only one when none is named.
        #[arg(long)]
        project: Option<String>,
        /// Reviewer agent profile; default `<worker profile>-reviewer`. An empty text clears it.
        #[arg(long)]
        reviewer: Option<String>,
    },
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
        /// The reviewer agent's output instead of the worker's.
        #[arg(long)]
        reviewer: bool,
    },
    /// Print every change live; resumes after a reconnect.
    Watch {
        /// Start after this event seq (0 = from the beginning).
        #[arg(long)]
        since: Option<i64>,
    },
    /// Print the web page address of the daemon.
    Open,
    /// Run the MCP server of an agent on stdio (the runner wires it in; needs the runner's
    /// environment).
    Mcp,
    /// Handle a Claude Code hook: read its JSON from stdin, record activity, report what is clear.
    Hook {
        /// The hook: notification, stop, subagent_stop, user_prompt_submit or post_tool_use.
        name: String,
    },
    /// Status line command of an agent: passes the plan limits Claude Code reports to the daemon
    /// (the runner wires it in; needs the runner's environment).
    Statusline,
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
    // The agent side finds the daemon in the runner's environment and never starts one.
    match cmd {
        Command::Mcp => mcp::run(),
        Command::Hook { name } => hook::run(&name),
        Command::Statusline => statusline::run(),
        cmd => run_on_home(cmd, json_out),
    }
}

fn run_on_home(cmd: Command, json_out: bool) -> Result<()> {
    let paths = Paths::resolve().map_err(anyhow::Error::msg)?;
    let c = crate::client::connect(&paths)?;
    match cmd {
        Command::Daemon(_) | Command::Mcp | Command::Hook { .. } | Command::Statusline => {
            unreachable!("handled above")
        }
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
        Command::Accept { ticket, comment } => {
            ticket::act(&c, &ticket, "accept", comment.as_deref(), json_out)
        }
        Command::ReviewMode {
            mode,
            ticket,
            chain,
            project,
            reviewer,
        } => review::run(
            &c,
            mode,
            review::Target::of(ticket, chain, project),
            reviewer,
            json_out,
        ),
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
        Command::Logs {
            ticket,
            follow,
            reviewer,
        } => logs::run(&c, &ticket, follow, reviewer),
        Command::Watch { since } => watch::run(&c, &paths, since, json_out),
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
