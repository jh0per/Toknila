//! `tok planner`: start and stop a project's planner, and read what it writes to its terminal.
//! The planner plans through its own tools; the owner talks to it in the window's terminal.

use anyhow::Result;
use clap::Subcommand;
use toknila_api_types::PlannerState;
use toknila_client::Client;

use super::{emit, logs};

/// `tok planner ...`
#[derive(Debug, Subcommand)]
pub enum PlannerCmd {
    /// Start the project's planner in its repository (read-only; it plans and starts nothing).
    Start {
        /// Project key.
        project: String,
        /// A planner profile (`tok profile ls`); default `claude-code-planner`.
        #[arg(long)]
        profile: Option<String>,
    },
    /// Stop the project's planner.
    Stop {
        /// Project key.
        project: String,
    },
    /// Print what the planner has written to its terminal; `--follow` keeps going live.
    Logs {
        /// Project key.
        project: String,
        /// Keep printing as the planner writes, until its run ends.
        #[arg(long, short)]
        follow: bool,
    },
}

fn say(project: &str, s: &PlannerState) {
    if s.running {
        println!("planner of {project} is running ({})", s.profile);
    } else {
        println!("planner of {project} is not running");
    }
}

/// What stopping did: the planner was running and is stopped now, or there was none.
fn stopped(project: &str, was_running: bool) -> String {
    if was_running {
        format!("planner of {project} stopped")
    } else {
        format!("planner of {project} was not running")
    }
}

pub(super) fn run(c: &Client, cmd: PlannerCmd, json_out: bool) -> Result<()> {
    match cmd {
        PlannerCmd::Start { project, profile } => {
            let s = c.start_planner(&project, profile.as_deref())?;
            emit(json_out, &s, |s| say(&project, s));
        }
        PlannerCmd::Stop { project } => {
            let was_running = c.planner(&project)?.running;
            let s = c.stop_planner(&project)?;
            emit(json_out, &s, |_| {
                println!("{}", stopped(&project, was_running))
            });
        }
        PlannerCmd::Logs { project, follow } => {
            let scrollback = c.planner_scrollback(&project)?;
            let url = c.planner_stream_url(&project, Some((scrollback.run, scrollback.end)));
            logs::print_and_follow(&scrollback, &url, follow)?;
        }
    }
    Ok(())
}
