//! The planner's cards: adding one and editing one that has not started, with the designs it
//! references. Both are the person's story commands ([`crate::as_caller`]) made as the planner;
//! these turn the planner's fields into the person's.

use serde::{Deserialize, Serialize};
use toknila_core::design::DesignRef;
use toknila_core::{ProjectId, TicketId, TicketPatch};

use crate::access::Caller;
use crate::as_caller;
use crate::tickets::NewTicket;
use crate::{AppResult, UnitOfWork, resolve};

pub use toknila_core::SubtaskEdit;

/// What the planner gives for a new card.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct NewCard {
    pub title: String,
    #[serde(default)]
    pub prompt: String,
    #[serde(default)]
    pub acceptance: Vec<String>,
    /// Cards (keys) or release versions of the chain it waits for. Empty: the end of the chain.
    #[serde(default)]
    pub after: Vec<String>,
    /// Checklist items.
    #[serde(default)]
    pub subtasks: Vec<String>,
    /// The card changes what the person sees: the agent shows a design for approval before any
    /// code.
    #[serde(default)]
    pub design_first: bool,
    /// Designs of the project the card is about, by file name or id.
    #[serde(default)]
    pub designs: Vec<DesignPick>,
    /// The card's own model: an alias or a full name; left out, the project's runs it.
    #[serde(default)]
    pub model: Option<String>,
}

/// A design the planner names: its file name (or id) in the project's library, and the part of
/// the image that matters. The host finds the design and gives the use case a [`DesignRef`].
pub use toknila_core::design::DesignPick;

impl NewCard {
    /// The person's new story with the planner's fields, its designs as the planner named them:
    /// at the end of the epic unless it names what it waits for.
    pub fn into_story(self) -> NewTicket {
        NewTicket {
            at_end: self.after.is_empty(),
            title: self.title,
            prompt: self.prompt,
            acceptance: self.acceptance,
            after: self.after,
            subtasks: self.subtasks,
            design_first: self.design_first,
            designs: self.designs,
            model: self.model,
            ..NewTicket::default()
        }
    }
}

/// Adds a card to a chain of the planner's project, without designs.
pub fn add_card<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    project: ProjectId,
    chain_ref: &str,
    input: NewCard,
) -> AppResult<TicketId> {
    add_card_with_designs(uow, project, chain_ref, input, Vec::new())
}

/// [`add_card`] with `designs`: `input.designs` resolved by the host against the project's
/// library.
pub fn add_card_with_designs<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    project: ProjectId,
    chain_ref: &str,
    input: NewCard,
    designs: Vec<DesignRef>,
) -> AppResult<TicketId> {
    let mut story = input.into_story();
    story.designs = designs.into_iter().map(DesignPick::from).collect();
    as_caller::add_story(uow, &Caller::Planner(project), chain_ref, story)
}

/// What the planner may change on a card that has not started.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct CardEdit {
    pub title: Option<String>,
    pub prompt: Option<String>,
    pub acceptance: Option<Vec<String>>,
    /// Whether the agent shows a design for approval before any code.
    pub design_first: Option<bool>,
    /// The card's own model, an alias or a full name; empty text clears it.
    #[serde(default)]
    pub model: Option<String>,
    /// The card's designs, replacing the list (`[]` clears it); left out keeps them.
    #[serde(default)]
    pub designs: Option<Vec<DesignPick>>,
    /// Checklist items to append.
    #[serde(default)]
    pub add_subtasks: Vec<String>,
    /// Checklist items to rename.
    #[serde(default)]
    pub edit_subtasks: Vec<SubtaskEdit>,
}

impl CardEdit {
    /// The person's story patch with the planner's fields, its designs as the planner named them
    /// (`Some` replaces the list).
    pub fn into_story(self) -> TicketPatch {
        TicketPatch {
            title: self.title,
            prompt: self.prompt,
            acceptance: self.acceptance,
            design_first: self.design_first,
            model: self.model,
            designs: self.designs,
            add_subtasks: self.add_subtasks,
            edit_subtasks: self.edit_subtasks,
            ..TicketPatch::default()
        }
    }
}

/// Edits a card of the planner's project that has not started.
pub fn edit_card<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    project: ProjectId,
    card_ref: &str,
    edit: CardEdit,
) -> AppResult<TicketId> {
    edit_card_with_designs(uow, project, card_ref, edit, None)
}

/// [`edit_card`] with `designs`: `edit.designs` resolved by the host (`Some` replaces the list).
pub fn edit_card_with_designs<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    project: ProjectId,
    card_ref: &str,
    edit: CardEdit,
    designs: Option<Vec<DesignRef>>,
) -> AppResult<TicketId> {
    let t = resolve::ticket(&*uow, card_ref)?;
    let mut patch = edit.into_story();
    patch.designs = designs.map(|refs| refs.into_iter().map(DesignPick::from).collect());
    as_caller::edit_story(uow, &Caller::Planner(project), &t, patch)
}
