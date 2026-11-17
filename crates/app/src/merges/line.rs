//! A project's merge line as the use cases keep it (the rules are
//! [`toknila_core::merge_line`]). A card's place is on its review, stamped by the machine; after
//! every change of an epic the free turn goes to the next one in line ([`advance`]): a card's
//! merge is queued only then, so no two cards of a project merge at once and a re-check runs
//! against a main that stays. A release run asks for its turn itself ([`take_release_turn`]).

use toknila_core::merge_line::{Entrant, Line, MergeWait, in_merge};
use toknila_core::{ProjectId, ReleaseId, Ticket};

use crate::{AppError, AppResult, Reader, UnitOfWork};

/// The project's tickets that are in their merge, from every epic.
fn in_line<R: Reader + ?Sized>(r: &R, project: ProjectId) -> AppResult<Vec<Ticket>> {
    let mut cards = Vec::new();
    for c in r.chains(Some(project))? {
        cards.extend(r.tickets(Some(c.id))?.into_iter().filter(in_merge));
    }
    Ok(cards)
}

/// The project's merge line now: its cards in their merge and its release runs.
pub fn line_of<R: Reader + ?Sized>(r: &R, project: ProjectId) -> AppResult<Line> {
    let cards = in_line(r, project)?;
    Ok(Line::of(&cards, &r.release_turns(project)?))
}

/// Gives the turn, when nobody holds it, to the next card in the project's line and queues its
/// merge. A release that is next takes the turn itself; nothing is queued for it.
pub fn advance<U: UnitOfWork + ?Sized>(uow: &mut U, project: ProjectId) -> AppResult<()> {
    let cards = in_line(uow, project)?;
    if cards.is_empty() {
        return Ok(());
    }
    let line = Line::of(&cards, &uow.release_turns(project)?);
    let Some(Entrant::Card { id, .. }) = line.next().cloned() else {
        return Ok(());
    };
    let Some(t) = cards.into_iter().find(|t| t.id == id) else {
        return Ok(());
    };
    crate::chain::run_chain(uow, t.chain_id, |ctx| {
        ctx.edit_ticket(id, |t| {
            if let Some(place) = t.review.merge_line.as_mut() {
                place.holds = true;
            }
        })
        .map_err(AppError::from)
    })?;
    let (Some(t), Some(chain)) = (uow.ticket(id)?, uow.chain(t.chain_id)?) else {
        return Ok(());
    };
    super::queue_merge(uow, &chain, &t)
}

/// The run of `release` asks for its turn in `project`'s line: `None` when it holds the turn now
/// (it took it, or had it), else what it waits for. It waits for the turn in progress only; the
/// cards that wait go after it.
pub fn take_release_turn<U: UnitOfWork + ?Sized>(
    uow: &mut U,
    project: ProjectId,
    release: ReleaseId,
) -> AppResult<Option<MergeWait>> {
    let line = line_of(uow, project)?;
    let me = |e: Option<&Entrant>| matches!(e, Some(Entrant::Release { id, .. }) if *id == release);
    if me(line.holder.as_ref()) {
        return Ok(None);
    }
    if me(line.next()) {
        uow.take_release_turn(release)?;
        return Ok(None);
    }
    Ok(line.release_wait(release))
}

/// After a start: a card in its merge that has no place in the line (a daemon before the line
/// left it so) joins it, oldest first, so the turn can go to it. Returns how many joined.
pub fn place_unplaced<U: UnitOfWork + ?Sized>(uow: &mut U) -> AppResult<usize> {
    let mut unplaced: Vec<Ticket> = Vec::new();
    for c in uow.chains(None)? {
        unplaced.extend(
            uow.tickets(Some(c.id))?
                .into_iter()
                .filter(|t| in_merge(t) && t.review.merge_line.is_none()),
        );
    }
    unplaced.sort_by_key(|t| (t.status_since, t.key.to_string()));
    for t in &unplaced {
        let since = t.status_since;
        crate::chain::run_chain(uow, t.chain_id, |ctx| {
            ctx.edit_ticket(t.id, |t| {
                t.review.merge_line = Some(toknila_core::merge_line::LinePlace {
                    since,
                    holds: false,
                });
            })
            .map_err(AppError::from)
        })?;
    }
    Ok(unplaced.len())
}
