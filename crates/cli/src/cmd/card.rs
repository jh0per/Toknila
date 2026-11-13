//! `tok card`: what a person does with a story's place.

use anyhow::Result;
use clap::Subcommand;
use toknila_api_types::{MoveInput, MoveToChain};
use toknila_client::Client;
use toknila_core::graph::Place;

use super::emit;
use crate::output;

#[derive(Debug, Subcommand)]
pub enum CardCmd {
    /// Move a story that has not started to another epic of its project: to the end of it, or
    /// right after (`--after`) or before (`--before`) a story of it. The key stays.
    Mv {
        /// The story's key (`BEATSTER-16`).
        key: String,
        /// The epic that receives it (name or id).
        #[arg(long)]
        chain: String,
        /// Put it right after this story of that epic.
        #[arg(long, conflicts_with = "before")]
        after: Option<String>,
        /// Put it right before this story of that epic.
        #[arg(long)]
        before: Option<String>,
    },
}

pub(super) fn run(c: &Client, cmd: CardCmd, json_out: bool) -> Result<()> {
    match cmd {
        CardCmd::Mv {
            key,
            chain,
            after,
            before,
        } => {
            let place = match (after, before) {
                (Some(anchor), _) => Some(MoveInput {
                    anchor,
                    place: Place::After,
                }),
                (None, Some(anchor)) => Some(MoveInput {
                    anchor,
                    place: Place::Before,
                }),
                (None, None) => None,
            };
            let input = MoveToChain { chain, place };
            emit(
                json_out,
                &c.move_to_chain(&key, &input)?,
                output::ticket_line,
            );
        }
    }
    Ok(())
}
