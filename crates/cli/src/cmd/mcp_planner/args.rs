//! The arguments of the planner's tools, as the model sends them.

use serde::Deserialize;
use toknila_api_types::{CardEdit, EditPlanChain, NewCard};
use toknila_core::graph::Place;
use toknila_core::{ArtifactFormat, ThreadKind};

#[derive(Deserialize)]
pub(super) struct AddCard {
    pub chain: String,
    #[serde(flatten)]
    pub card: NewCard,
}

#[derive(Deserialize)]
pub(super) struct EditCard {
    pub card: String,
    #[serde(flatten)]
    pub edit: CardEdit,
}

#[derive(Deserialize)]
pub(super) struct EditChain {
    pub chain: String,
    #[serde(flatten)]
    pub edit: EditPlanChain,
}

#[derive(Deserialize)]
pub(super) struct ShowCard {
    pub card: String,
}

#[derive(Deserialize)]
pub(super) struct ShowDesign {
    pub design: String,
}

#[derive(Deserialize)]
pub(super) struct Dependency {
    pub chain: String,
    pub from: String,
    pub to: String,
}

#[derive(Deserialize)]
pub(super) struct Move {
    pub chain: String,
    pub card: String,
    #[serde(default)]
    pub anchor: Option<String>,
    #[serde(default)]
    pub place: Option<Place>,
    #[serde(default)]
    pub to_chain: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct OpenThread {
    pub card: String,
    pub kind: ThreadKind,
    pub title: String,
}

#[derive(Deserialize)]
pub(super) struct ThreadMessage {
    pub card: String,
    pub thread: String,
    pub text: String,
}

#[derive(Deserialize)]
pub(super) struct ThreadVersion {
    pub card: String,
    pub thread: String,
    pub format: ArtifactFormat,
    pub body: String,
}
