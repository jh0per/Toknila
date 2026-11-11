//! `tok release`.

use anyhow::{Context, Result};
use clap::Subcommand;
use toknila_api_types::{NewRelease, ReleaseRun, RunState, StepState};
use toknila_client::Client;
use toknila_core::StartMode;
use toknila_core::wire::from_wire_name;

use super::emit;
use crate::output;

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
    /// Release a finished chain: runs the check, the bump, the tag and the push.
    Run {
        chain: String,
        version: String,
        /// Run an interrupted or failed release again from its step; passed steps are not redone.
        #[arg(long)]
        resume: bool,
    },
    /// Show the steps of a release's last run, as the journal has them (also after a restart).
    Steps { release: String },
}

/// The steps of a run, one line each, then why it stopped.
fn steps(run: &ReleaseRun) {
    println!("Release {} ({}): {}", run.version, run.tag, state_word(run));
    for s in &run.steps {
        let mark = match s.state {
            StepState::Pending => "o",
            StepState::Running => "…",
            StepState::Done => "ok",
            StepState::Failed => "FAILED",
            StepState::Interrupted => "INTERRUPTED",
        };
        println!("  {mark:<11} {}", s.label);
    }
    if let Some(e) = &run.error {
        println!("{e}");
    }
    if let Some(step) = run.resume_from {
        println!(
            "Interrupted at {}: tok release run <chain> {} --resume",
            step.label(),
            run.version
        );
    }
}

fn state_word(run: &ReleaseRun) -> &'static str {
    match run.state {
        RunState::Running => "running",
        RunState::Failed => "failed",
        RunState::Done => "released",
        RunState::Interrupted => "interrupted",
    }
}

pub(super) fn run(c: &Client, cmd: ReleaseCmd, json_out: bool) -> Result<()> {
    let detail = match cmd {
        ReleaseCmd::Steps { release } => {
            emit(json_out, &c.release_run(&release)?, steps);
            return Ok(());
        }
        ReleaseCmd::Run {
            chain,
            version,
            resume,
        } => c.start_release(&chain, &version, resume)?,
        ReleaseCmd::New {
            chain,
            version,
            channel,
            auto,
            after,
            before,
        } => {
            let input = NewRelease {
                version,
                channel: Some(from_wire_name(&channel).context("unknown channel")?),
                start_mode: Some(if auto {
                    StartMode::Auto
                } else {
                    StartMode::Manual
                }),
                after,
                before,
            };
            c.create_release(&chain, &input)?
        }
        ReleaseCmd::Start { release } => c.release_action(&release, "start")?,
    };
    emit(json_out, &detail, output::chain);
    Ok(())
}
