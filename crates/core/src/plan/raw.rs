//! The chain file as TOML gives it, before validation.

use serde::Deserialize;
use toml::Spanned;

use crate::{ReleaseChannel, ReviewMode, StartMode};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawFile {
    pub(super) chain: RawChain,
    #[serde(default)]
    pub(super) ticket: Vec<RawTicket>,
    #[serde(default)]
    pub(super) release: Vec<RawRelease>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawChain {
    pub(super) name: String,
    #[serde(default)]
    pub(super) goal: String,
    pub(super) autostart: Option<bool>,
    pub(super) review_mode: Option<ReviewMode>,
    /// The version the chain releases (`0.3.0`).
    pub(super) version: Option<String>,
    /// File names of the project's designs, as an export writes them; read and ignored.
    #[serde(default)]
    #[allow(dead_code)]
    pub(super) designs: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawTicket {
    pub(super) id: Spanned<String>,
    pub(super) title: Option<String>,
    #[serde(default)]
    pub(super) prompt: String,
    #[serde(default)]
    pub(super) acceptance: Vec<String>,
    #[serde(default)]
    pub(super) subtasks: Vec<String>,
    #[serde(default)]
    pub(super) after: Vec<Spanned<String>>,
    #[serde(default)]
    pub(super) after_release: Vec<Spanned<String>>,
    pub(super) profile: Option<String>,
    pub(super) review_mode: Option<ReviewMode>,
    #[serde(default)]
    pub(super) needs_secrets: bool,
    #[serde(default)]
    pub(super) design_first: bool,
    #[serde(default)]
    pub(super) paused: bool,
    /// Import the ticket as Merged (through the state machine, once its dependencies are).
    #[serde(default)]
    pub(super) done: bool,
    /// 1-based positions of the subtasks that are already ticked.
    #[serde(default)]
    pub(super) done_subtasks: Vec<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawRelease {
    pub(super) id: Spanned<String>,
    pub(super) version: Option<String>,
    pub(super) channel: Option<ReleaseChannel>,
    pub(super) start: Option<StartMode>,
    #[serde(default)]
    pub(super) after: Vec<Spanned<String>>,
    #[serde(default)]
    pub(super) before: Vec<Spanned<String>>,
}
