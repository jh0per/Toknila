//! Every `tok` subcommand: parse, call the daemon, print a table or the raw JSON.

use anyhow::{Context, Result, bail};
use clap::{Args, Subcommand};
use serde_json::{Value, json};
use toknila_daemon::paths::Paths;

use crate::client::{Client, enc};
use crate::{daemon_cmd, output};

/// Top-level commands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Start, stop or inspect the background daemon.
    #[command(subcommand)]
    Daemon(daemon_cmd::DaemonCmd),
    /// Projects: one product, one repository.
    #[command(subcommand)]
    Project(ProjectCmd),
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
    /// Print every change live; resumes after a reconnect.
    Watch {
        /// Start after this event seq (0 = from the beginning).
        #[arg(long)]
        since: Option<i64>,
    },
    /// Print the web page address of the daemon.
    Open,
}

#[derive(Debug, Subcommand)]
pub enum ProjectCmd {
    /// Create a project; the key prefixes ticket keys (demo -> DEMO-1).
    New {
        key: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        repo: Option<String>,
    },
    /// List projects.
    Ls,
}

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
    /// Create a chain with tickets, dependencies and milestones from a TOML file.
    Import {
        file: std::path::PathBuf,
        #[arg(long)]
        project: Option<String>,
    },
    /// Write a chain as TOML (to stdout, or a file).
    Export {
        chain: String,
        file: Option<std::path::PathBuf>,
    },
    /// Show a chain as a graph in dependency order with its status.
    Show { chain: String },
    /// List chains with their status.
    Ls,
}

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
    },
    /// Show a ticket.
    Show { ticket: String },
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

#[derive(Debug, Subcommand)]
pub enum SubtaskCmd {
    /// Add a checklist item.
    Add { ticket: String, text: String },
    /// Tick a checklist item (1-based position).
    Done { ticket: String, position: u32 },
}

#[derive(Debug, Subcommand)]
pub enum ReleaseCmd {
    /// Add a milestone to a chain.
    New {
        chain: String,
        version: String,
        #[arg(long, value_parser = ["tag_only", "test_flight", "play_internal", "play_closed", "play_production"], default_value = "tag_only")]
        channel: String,
        /// Cut automatically when everything before it is done (default: manual).
        #[arg(long)]
        auto: bool,
        /// What the milestone waits for; repeatable.
        #[arg(long)]
        after: Vec<String>,
        /// What waits for the milestone; repeatable.
        #[arg(long)]
        before: Vec<String>,
    },
    /// Cut a manual milestone (release/<version> leaves develop).
    Start { release: String },
}

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

/// Prints `v` as JSON, or with the human formatter.
fn emit(json_out: bool, v: &Value, human: impl FnOnce(&Value)) {
    if json_out {
        println!("{}", serde_json::to_string_pretty(v).unwrap_or_default());
    } else {
        human(v);
    }
}

fn ticket_line(v: &Value) {
    let t = &v["ticket"];
    let r = output::reason(&t["status_reason"]);
    println!(
        "{} {}{}",
        t["key"].as_str().unwrap_or(""),
        t["status"].as_str().unwrap_or(""),
        if r.is_empty() {
            String::new()
        } else {
            format!(" ({r})")
        }
    );
}

/// Project to use when none is given: the only one there is.
fn default_project(c: &Client, given: Option<String>) -> Result<String> {
    if let Some(p) = given {
        return Ok(p);
    }
    let projects = c.get("/projects")?;
    match projects.as_array().map(Vec::as_slice) {
        Some([one]) => Ok(one["key"].as_str().unwrap_or_default().to_owned()),
        Some([]) | None => bail!("no project yet; create one with `tok project new <key>`"),
        Some(_) => bail!("several projects exist; pass --project <key>"),
    }
}

fn ticket_action(
    c: &Client,
    ticket: &str,
    action: &str,
    text: Option<&str>,
    json_out: bool,
) -> Result<()> {
    let v = c.post(
        &format!("/tickets/{}/actions/{action}", enc(ticket)),
        json!({ "text": text }),
    )?;
    emit(json_out, &v, ticket_line);
    Ok(())
}

fn chain_of(c: &Client, refs: &[&str], given: Option<String>) -> Result<String> {
    if let Some(chain) = given {
        return Ok(chain);
    }
    for r in refs {
        if let Ok(t) = c.get(&format!("/tickets/{}", enc(r))) {
            return Ok(t["ticket"]["chain_id"]
                .as_str()
                .unwrap_or_default()
                .to_owned());
        }
    }
    bail!(
        "cannot tell the chain from {}; pass --chain",
        refs.join(" and ")
    )
}

pub fn run(cmd: Command, json_out: bool) -> Result<()> {
    if let Command::Daemon(d) = cmd {
        return daemon_cmd::run(d, json_out);
    }
    let paths = Paths::resolve().map_err(anyhow::Error::msg)?;
    let c = Client::connect(&paths)?;
    match cmd {
        Command::Daemon(_) => unreachable!("handled above"),
        Command::Project(ProjectCmd::New { key, name, repo }) => {
            let v = c.post("/projects", json!({ "key": key.to_uppercase(), "name": name.unwrap_or(key), "repo_path": repo }))?;
            emit(json_out, &v, |v| {
                println!("project {} created", v["key"].as_str().unwrap_or(""))
            });
        }
        Command::Project(ProjectCmd::Ls) => {
            let v = c.get("/projects")?;
            emit(json_out, &v, |v| {
                let rows: Vec<Vec<String>> = v
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|p| {
                        vec![
                            p["key"].as_str().unwrap_or("").to_owned(),
                            p["name"].as_str().unwrap_or("").to_owned(),
                        ]
                    })
                    .collect();
                output::table(&["KEY", "NAME"], &rows);
            });
        }
        Command::Chain(ChainCmd::New {
            name,
            goal,
            no_autostart,
            project,
        }) => {
            let p = default_project(&c, project)?;
            let v = c.post(
                &format!("/projects/{}/chains", enc(&p)),
                json!({ "name": name, "goal": goal, "autostart": !no_autostart }),
            )?;
            emit(json_out, &v, output::chain);
        }
        Command::Chain(ChainCmd::Import { file, project }) => {
            let text = std::fs::read_to_string(&file)
                .with_context(|| format!("read {}", file.display()))?;
            let p = default_project(&c, project)?;
            let v = c.post(
                &format!("/projects/{}/import", enc(&p)),
                json!({ "toml": text }),
            )?;
            emit(json_out, &v, output::chain);
        }
        Command::Chain(ChainCmd::Export { chain, file }) => {
            let v = c.get(&format!("/chains/{}/export", enc(&chain)))?;
            let toml = v["toml"].as_str().unwrap_or_default();
            match file {
                Some(f) => {
                    std::fs::write(&f, toml).with_context(|| format!("write {}", f.display()))?
                }
                None => print!("{toml}"),
            }
        }
        Command::Chain(ChainCmd::Show { chain }) => {
            let v = c.get(&format!("/chains/{}", enc(&chain)))?;
            emit(json_out, &v, output::chain);
        }
        Command::Chain(ChainCmd::Ls) => {
            let v = c.get("/chains")?;
            emit(json_out, &v, |v| {
                let rows: Vec<Vec<String>> = v
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|it| {
                        let s = &it["summary"];
                        vec![
                            it["chain"]["name"].as_str().unwrap_or("").to_owned(),
                            s["status"].as_str().unwrap_or("").to_owned(),
                            format!("{}/{}", s["done"], s["total"]),
                            s["stopped_at"]["ticket"].as_str().unwrap_or("").to_owned(),
                        ]
                    })
                    .collect();
                output::table(&["CHAIN", "STATUS", "DONE", "STOPPED AT"], &rows);
            });
        }
        Command::Ticket(TicketCmd::New {
            chain,
            title,
            after,
            profile,
            prompt,
            accept,
            subtask,
        }) => {
            let body = json!({
                "title": title, "after": after, "profile": profile, "prompt": prompt.unwrap_or_default(),
                "acceptance": accept, "subtasks": subtask,
            });
            let v = c.post(&format!("/chains/{}/tickets", enc(&chain)), body)?;
            emit(json_out, &v, ticket_line);
        }
        Command::Ticket(TicketCmd::Show { ticket }) | Command::Show { ticket } => {
            let v = c.get(&format!("/tickets/{}", enc(&ticket)))?;
            emit(json_out, &v, output::ticket);
        }
        Command::Dep(DepCmd::Add {
            args,
            after_release,
        }) => {
            let chain = chain_of(&c, &[&args.to, &args.from], args.chain)?;
            let kind = if after_release {
                "after_release"
            } else {
                "normal"
            };
            let v = c.post(
                &format!("/chains/{}/edges", enc(&chain)),
                json!({ "from": args.from, "to": args.to, "kind": kind }),
            )?;
            emit(json_out, &v, output::chain);
        }
        Command::Dep(DepCmd::Rm { args }) => {
            let chain = chain_of(&c, &[&args.to, &args.from], args.chain)?;
            let v = c.delete(&format!(
                "/chains/{}/edges?from={}&to={}",
                enc(&chain),
                enc(&args.from),
                enc(&args.to)
            ))?;
            emit(json_out, &v, output::chain);
        }
        Command::Subtask(SubtaskCmd::Add { ticket, text }) => {
            let v = c.post(
                &format!("/tickets/{}/subtasks", enc(&ticket)),
                json!({ "text": text }),
            )?;
            emit(json_out, &v, output::ticket);
        }
        Command::Subtask(SubtaskCmd::Done { ticket, position }) => {
            let v = c.patch(
                &format!("/tickets/{}/subtasks/{position}", enc(&ticket)),
                json!({ "done": true }),
            )?;
            emit(json_out, &v, output::ticket);
        }
        Command::Comment { ticket, text, kind } => {
            let v = c.post(
                &format!("/tickets/{}/comments", enc(&ticket)),
                json!({ "text": text, "kind": kind }),
            )?;
            emit(json_out, &v, ticket_line);
        }
        Command::Start { ticket } => ticket_action(&c, &ticket, "start", None, json_out)?,
        Command::Pause { ticket } => ticket_action(&c, &ticket, "pause", None, json_out)?,
        Command::Resume { ticket } => ticket_action(&c, &ticket, "resume", None, json_out)?,
        Command::Block { ticket, reason } => {
            ticket_action(&c, &ticket, "block", Some(&reason), json_out)?
        }
        Command::Unblock { ticket } => ticket_action(&c, &ticket, "unblock", None, json_out)?,
        Command::Accept { ticket } => ticket_action(&c, &ticket, "accept", None, json_out)?,
        Command::Return { ticket, comment } => {
            ticket_action(&c, &ticket, "return", Some(&comment), json_out)?
        }
        Command::Reopen { ticket } => ticket_action(&c, &ticket, "reopen", None, json_out)?,
        Command::Cancel { ticket, reason } => {
            ticket_action(&c, &ticket, "cancel", Some(&reason), json_out)?
        }
        Command::Release(ReleaseCmd::New {
            chain,
            version,
            channel,
            auto,
            after,
            before,
        }) => {
            let body = json!({
                "version": version, "channel": channel, "start_mode": if auto { "auto" } else { "manual" },
                "after": after, "before": before,
            });
            let v = c.post(&format!("/chains/{}/releases", enc(&chain)), body)?;
            emit(json_out, &v, output::chain);
        }
        Command::Release(ReleaseCmd::Start { release }) => {
            let v = c.post(
                &format!("/releases/{}/actions/start", enc(&release)),
                json!({}),
            )?;
            emit(json_out, &v, output::chain);
        }
        Command::Sim { target, action } => sim(&c, &target, action, json_out)?,
        Command::Ls { needs_human, chain } => {
            let mut q = format!("/tickets?needs_human={needs_human}");
            if let Some(ch) = chain {
                q.push_str(&format!("&chain={}", enc(&ch)));
            }
            let v = c.get(&q)?;
            emit(json_out, &v, output::tickets);
        }
        Command::Watch { since } => watch(&c, &paths, since)?,
        Command::Open => {
            let info = daemon_cmd::running(&paths)?.context("daemon is not running")?;
            let url = format!("{}/?token={}", info.base_url(), info.token);
            emit(json_out, &json!({ "url": url }), |_| println!("{url}"));
        }
    }
    Ok(())
}

fn sim(c: &Client, target: &str, action: SimCmd, json_out: bool) -> Result<()> {
    let ticket_event = |body: Value| -> Result<()> {
        let v = c.post(&format!("/tickets/{}/events", enc(target)), body)?;
        emit(json_out, &v, ticket_line);
        Ok(())
    };
    match action {
        SimCmd::Start => ticket_event(json!({ "event": "started" })),
        SimCmd::Ask { text, permission } => {
            ticket_event(json!({ "event": "ask_human", "text": text, "permission": permission }))
        }
        SimCmd::Subtask { position, .. } => {
            ticket_event(json!({ "event": "subtask_done", "position": position }))
        }
        SimCmd::Note { text } => ticket_event(json!({ "event": "note", "body": { "gist": text } })),
        SimCmd::Crash { detail } => ticket_event(json!({ "event": "crashed", "detail": detail })),
        SimCmd::Finish { summary } => {
            // A ticket first; if no ticket has this name, a release.
            match c.get(&format!("/tickets/{}", enc(target))) {
                Ok(_) => {
                    let summary = summary.context("finish needs a summary for a ticket")?;
                    ticket_event(json!({ "event": "finished", "summary": summary }))
                }
                Err(_) => {
                    let v = c.post(
                        &format!("/releases/{}/events", enc(target)),
                        json!({ "event": "finished" }),
                    )?;
                    emit(json_out, &v, output::chain);
                    Ok(())
                }
            }
        }
    }
}

fn watch(c: &Client, paths: &Paths, since: Option<i64>) -> Result<()> {
    let mut last = match since {
        Some(s) => s,
        // Default: only what happens from now on.
        None => c
            .get("/events?since=0&limit=1000000")?
            .as_array()
            .and_then(|a| a.last())
            .and_then(|e| e["seq"].as_i64())
            .unwrap_or(0),
    };
    let mut quiet = false;
    loop {
        // Re-read daemon.json each time: a restarted daemon has a new port and token.
        let url = daemon_cmd::running(paths)?.map(|info| {
            format!(
                "ws://127.0.0.1:{}/v1/stream?since={last}&token={}",
                info.port, info.token
            )
        });
        let conn = url.as_deref().map(tungstenite::connect);
        match conn {
            Some(Ok((mut ws, _))) => {
                quiet = false;
                loop {
                    match ws.read() {
                        Ok(tungstenite::Message::Text(t)) => {
                            let e: Value = serde_json::from_str(t.as_str())?;
                            if let Some(seq) = e["seq"].as_i64() {
                                last = seq;
                            }
                            println!("{}", output::event(&e));
                        }
                        Ok(tungstenite::Message::Close(_)) | Err(_) => break,
                        Ok(_) => {}
                    }
                }
            }
            _ => {
                if !quiet {
                    eprintln!("(no daemon; waiting to reconnect after #{last})");
                    quiet = true;
                }
                std::thread::sleep(std::time::Duration::from_millis(500));
            }
        }
    }
}
