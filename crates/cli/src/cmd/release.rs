//! `tok release`.

use anyhow::{Context, Result};
use clap::Subcommand;
use toknila_api_types::NewRelease;
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
}

pub(super) fn run(c: &Client, cmd: ReleaseCmd, json_out: bool) -> Result<()> {
    let detail = match cmd {
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
