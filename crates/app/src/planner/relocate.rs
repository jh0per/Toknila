//! The planner's `move_card`: a card goes next to another of its epic, or to another epic of the
//! project. Both are the person's moves ([`crate::as_caller`]) made as the planner.

use toknila_core::graph::Place;
use toknila_core::{ChainId, ProjectId};

use super::move_card;
use crate::access::Caller;
use crate::{AppError, AppResult, UnitOfWork, as_caller};

/// Moves a card that has not started from `chain_ref` to another epic of the planner's project,
/// to the end or next to `place`'s anchor. Neither epic may start its cards by itself. Returns the
/// epic that received the card.
pub fn move_card_to_chain<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    project: ProjectId,
    chain_ref: &str,
    card_ref: &str,
    to_chain: &str,
    place: Option<(&str, Place)>,
) -> AppResult<ChainId> {
    let caller = Caller::Planner(project);
    as_caller::move_story_to_epic(uow, &caller, card_ref, to_chain, place, Some(chain_ref))
}

/// The anchor and place a move names: both, or neither.
fn anchor_and_place(
    anchor: Option<&str>,
    place: Option<Place>,
) -> AppResult<Option<(&str, Place)>> {
    match (anchor, place) {
        (Some(a), Some(p)) => Ok(Some((a, p))),
        (None, None) => Ok(None),
        _ => Err(AppError::bad_request(
            "give both anchor and place, or neither",
        )),
    }
}

/// What `POST /planner/chains/{chain}/cards/{card}/move` asks: with `to_chain` the card changes
/// epic (to the end, or next to `anchor` when `place` says where); without it `anchor` and `place`
/// put it next to a card of its own epic. Returns the epic the card is in afterwards.
pub fn move_card_by<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    project: ProjectId,
    chain_ref: &str,
    card_ref: &str,
    to: (Option<&str>, Option<&str>, Option<Place>),
) -> AppResult<ChainId> {
    let (to_chain, anchor, place) = to;
    let at = anchor_and_place(anchor, place)?;
    match (to_chain, at) {
        (Some(to), at) => move_card_to_chain(uow, project, chain_ref, card_ref, to, at),
        (None, Some((a, p))) => move_card(uow, project, chain_ref, card_ref, a, p),
        (None, None) => Err(AppError::bad_request("give anchor and place, or to_chain")),
    }
}
