//! The planner's `move_card`: a card goes next to another of its epic, or to another epic of the
//! project.

use toknila_core::graph::Place;
use toknila_core::planner;
use toknila_core::{ChainId, ProjectId};

use super::{card, chain, move_card};
use crate::{AppError, AppResult, UnitOfWork};

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
    let from = chain(uow, project, chain_ref)?;
    let to = chain(uow, project, to_chain)?;
    planner::ensure_graph_change(&from)?;
    planner::ensure_graph_change(&to)?;
    let t = card(uow, project, card_ref)?;
    if t.chain_id != from.id {
        return Err(AppError::bad_request(format!(
            "{} is not in epic {}",
            t.key, from.name
        )));
    }
    crate::moves::move_to_chain(uow, &t, &to.id.to_string(), place)?;
    Ok(to.id)
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
    let at = match (anchor, place) {
        (Some(a), Some(p)) => Some((a, p)),
        (None, None) => None,
        _ => {
            return Err(AppError::bad_request(
                "give both anchor and place, or neither",
            ));
        }
    };
    match (to_chain, at) {
        (Some(to), at) => move_card_to_chain(uow, project, chain_ref, card_ref, to, at),
        (None, Some((a, p))) => move_card(uow, project, chain_ref, card_ref, a, p),
        (None, None) => Err(AppError::bad_request("give anchor and place, or to_chain")),
    }
}
