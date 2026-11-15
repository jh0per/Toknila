//! `tok release`.

use anyhow::{Context, Result};
use clap::Subcommand;
use toknila_api_types::{NewRelease, ReleaseRun, RunState, StepState};
use toknila_client::Client;
use toknila_core::StartMode;
use toknila_core::release_run::ReleaseStep;
use toknila_core::wire::from_wire_name;

use super::emit;
use crate::output;

#[derive(Debug, Subcommand)]
pub enum ReleaseCmd {
    /// Add a milestone to an epic.
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
    /// Release a finished epic: runs the check, the bump, the tag and the push.
    Run {
        chain: String,
        version: String,
        /// Run an interrupted or failed release again from its step; passed steps are not redone.
        #[arg(long)]
        resume: bool,
        /// Push the base branch and the tag to the remote (the default when the base has an
        /// upstream or the project merges by pull request).
        #[arg(long, conflicts_with = "no_push")]
        push: bool,
        /// Keep the tag local: push nothing.
        #[arg(long)]
        no_push: bool,
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
        // What a passed push says besides the sentence: the tag's page.
        if s.step == ReleaseStep::Push && s.state == StepState::Done {
            for line in s.output.lines().skip(1) {
                println!("  {:<11} {line}", "");
            }
        }
    }
    if let Some(e) = &run.error {
        println!("{e}");
    }
    if let Some(step) = run.resume_from {
        println!(
            "Interrupted at {}: tok release run <epic> {} --resume",
            step.label(),
            run.version
        );
    }
}

/// `--push` is `Some(true)`, `--no-push` is `Some(false)`, neither leaves the choice to the daemon.
fn push_choice(push: bool, no_push: bool) -> Option<bool> {
    match (push, no_push) {
        (true, _) => Some(true),
        (_, true) => Some(false),
        _ => None,
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
            push,
            no_push,
        } => c.start_release(&chain, &version, resume, push_choice(push, no_push))?,
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
