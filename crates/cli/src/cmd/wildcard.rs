//! `tok propose`: a person proposes a wildcard card (work nobody planned). `tok approve` and
//! `tok reject` are ticket actions; `tok sim <ticket> propose` is the agent's way in.

use anyhow::Result;
use clap::Args;
use toknila_api_types::{NewWildcard, Proposal};
use toknila_client::Client;

use super::emit;
use crate::output;

/// What a wildcard card is and where it would go in the graph.
#[derive(Debug, Args)]
pub struct CardArgs {
    title: String,
    #[arg(long)]
    prompt: Option<String>,
    /// Acceptance criterion; repeatable.
    #[arg(long)]
    accept: Vec<String>,
    /// What the story waits for (ticket keys, titles or release versions); repeatable.
    #[arg(long)]
    after: Vec<String>,
    /// What waits for the story; none of it may have started when the story is approved.
    /// Repeatable.
    #[arg(long)]
    before: Vec<String>,
}

impl From<CardArgs> for Proposal {
    fn from(a: CardArgs) -> Self {
        Self {
            title: a.title,
            prompt: a.prompt.unwrap_or_default(),
            acceptance: a.accept,
            after: a.after,
            before: a.before,
        }
    }
}

/// `tok propose <chain> <title> ...`.
pub(super) fn propose(
    c: &Client,
    chain: &str,
    card: CardArgs,
    from: Option<String>,
    json_out: bool,
) -> Result<()> {
    let input = NewWildcard::new(card.into(), from);
    emit(
        json_out,
        &c.propose_wildcard(chain, &input)?,
        output::ticket_line,
    );
    Ok(())
}
