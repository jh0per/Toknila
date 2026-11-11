//! The planner's cards: adding one and editing one that has not started, with the designs it
//! references.

use serde::{Deserialize, Serialize};
use toknila_core::design::DesignRef;
use toknila_core::planner;
use toknila_core::{ProjectId, TicketId, TicketPatch};

use super::{card, chain};
use crate::tickets;
use crate::tickets::NewTicket;
use crate::{AppResult, UnitOfWork, notes};

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
}

/// A design the planner names: its file name (or id) in the project's library, and the part of
/// the image that matters. The host finds the design and gives the use case a [`DesignRef`].
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DesignPick {
    pub design: String,
    /// In whole percent of the image; left out for the whole image.
    #[serde(default)]
    pub region: Option<toknila_core::design::Region>,
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
    let c = chain(uow, project, chain_ref)?;
    planner::ensure_graph_change(&c)?;
    let at_end = input.after.is_empty();
    let input = NewTicket {
        title: input.title,
        prompt: input.prompt,
        acceptance: input.acceptance,
        after: input.after,
        subtasks: input.subtasks,
        design_first: input.design_first,
        designs,
        at_end,
        ..NewTicket::default()
    };
    tickets::create(uow, &c.id.to_string(), input)
}

/// A rename of a checklist item.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SubtaskEdit {
    /// 1-based position.
    pub position: u32,
    pub text: String,
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
    let t = card(uow, project, card_ref)?;
    planner::ensure_card_editable(&t)?;
    let patch = TicketPatch {
        title: edit.title,
        prompt: edit.prompt,
        acceptance: edit.acceptance,
        design_first: edit.design_first,
        designs,
        ..TicketPatch::default()
    };
    if patch != TicketPatch::default() {
        tickets::edit(uow, &t, patch)?;
    }
    for item in &edit.edit_subtasks {
        notes::update_subtask(uow, &t, item.position, None, Some(item.text.clone()))?;
    }
    for text in &edit.add_subtasks {
        notes::add_subtask(uow, &t, text)?;
    }
    Ok(t.id)
}
